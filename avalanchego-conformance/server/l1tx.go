// Copyright (C) 2019-2022, Ava Labs, Inc. All rights reserved.
// See the file LICENSE for licensing terms.

package server

import (
	"bytes"
	"context"
	"fmt"

	"github.com/ava-labs/avalanche-rs/avalanchego-conformance/rpcpb"
	"github.com/ava-labs/avalanchego/ids"
	"github.com/ava-labs/avalanchego/utils/crypto/bls"
	"github.com/ava-labs/avalanchego/utils/hashing"
	"github.com/ava-labs/avalanchego/vms/components/avax"
	"github.com/ava-labs/avalanchego/vms/platformvm/signer"
	"github.com/ava-labs/avalanchego/vms/platformvm/txs"
	"github.com/ava-labs/avalanchego/vms/platformvm/warp"
	"github.com/ava-labs/avalanchego/vms/platformvm/warp/message"
	"github.com/ava-labs/avalanchego/vms/secp256k1fx"
	"go.uber.org/zap"
)

// ============================================================================
// L1 Transaction Handlers
// ============================================================================

func (s *server) ConvertSubnetToL1Tx(ctx context.Context, req *rpcpb.ConvertSubnetToL1TxRequest) (*rpcpb.ConvertSubnetToL1TxResponse, error) {
	zap.L().Debug("received ConvertSubnetToL1Tx request")

	// Build the transaction using avalanchego types
	subnetID, err := ids.ToID(req.SubnetId)
	if err != nil {
		return nil, fmt.Errorf("invalid subnet_id: %w", err)
	}

	chainID, err := ids.ToID(req.ChainId)
	if err != nil {
		return nil, fmt.Errorf("invalid chain_id: %w", err)
	}

	// Convert initial validators
	validators := make([]*txs.ConvertSubnetToL1Validator, len(req.Validators))
	for i, v := range req.Validators {
		// Signer contains both public key and proof of possession
		var pop signer.ProofOfPossession
		copy(pop.PublicKey[:], v.BlsPublicKey)
		// Note: For conformance testing, the PoP signature would need to be provided
		// In actual usage, this would be generated from the BLS secret key

		validators[i] = &txs.ConvertSubnetToL1Validator{
			NodeID:                v.NodeId, // types.JSONByteSlice
			Weight:                v.Weight,
			Balance:               0, // Initial balance is 0 for ConvertSubnetToL1
			Signer:                pop,
			RemainingBalanceOwner: convertToPChainOwner(v.RemainingBalanceOwner),
			DeactivationOwner:     convertToPChainOwner(v.DeactivationOwner),
		}
	}

	// Build the unsigned transaction
	utx := &txs.ConvertSubnetToL1Tx{
		BaseTx:     buildBaseTx(req.NetworkId, req.BlockchainId, req.Outputs, req.Inputs, req.Memo),
		Subnet:     subnetID,
		ChainID:    chainID,
		Address:    req.Address, // types.JSONByteSlice
		Validators: validators,
		SubnetAuth: &secp256k1fx.Input{},
	}

	// Serialize the transaction
	signedTx, err := txs.NewSigned(utx, txs.Codec, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create signed tx: %w", err)
	}

	expectedBytes := signedTx.Bytes()
	txID := signedTx.ID()

	resp := &rpcpb.ConvertSubnetToL1TxResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    txID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedTx, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) RegisterL1ValidatorTx(ctx context.Context, req *rpcpb.RegisterL1ValidatorTxRequest) (*rpcpb.RegisterL1ValidatorTxResponse, error) {
	zap.L().Debug("received RegisterL1ValidatorTx request")

	// Build the serialized warp message
	warpMsgBytes, err := buildWarpMessageBytes(req.WarpMessage)
	if err != nil {
		return nil, fmt.Errorf("failed to build warp message: %w", err)
	}

	// Build the unsigned transaction
	utx := &txs.RegisterL1ValidatorTx{
		BaseTx:            buildBaseTx(req.NetworkId, req.BlockchainId, req.Outputs, req.Inputs, req.Memo),
		Balance:           req.Balance,
		ProofOfPossession: *(*[bls.SignatureLen]byte)(req.BlsProofOfPossession),
		Message:           warpMsgBytes, // types.JSONByteSlice
	}

	// Serialize the transaction
	signedTx, err := txs.NewSigned(utx, txs.Codec, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create signed tx: %w", err)
	}

	expectedBytes := signedTx.Bytes()
	txID := signedTx.ID()

	resp := &rpcpb.RegisterL1ValidatorTxResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    txID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedTx, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) SetL1ValidatorWeightTx(ctx context.Context, req *rpcpb.SetL1ValidatorWeightTxRequest) (*rpcpb.SetL1ValidatorWeightTxResponse, error) {
	zap.L().Debug("received SetL1ValidatorWeightTx request")

	// Build the serialized warp message
	warpMsgBytes, err := buildWarpMessageBytes(req.WarpMessage)
	if err != nil {
		return nil, fmt.Errorf("failed to build warp message: %w", err)
	}

	// Build the unsigned transaction
	utx := &txs.SetL1ValidatorWeightTx{
		BaseTx:  buildBaseTx(req.NetworkId, req.BlockchainId, req.Outputs, req.Inputs, req.Memo),
		Message: warpMsgBytes, // types.JSONByteSlice
	}

	// Serialize the transaction
	signedTx, err := txs.NewSigned(utx, txs.Codec, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create signed tx: %w", err)
	}

	expectedBytes := signedTx.Bytes()
	txID := signedTx.ID()

	resp := &rpcpb.SetL1ValidatorWeightTxResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    txID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedTx, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) IncreaseL1ValidatorBalanceTx(ctx context.Context, req *rpcpb.IncreaseL1ValidatorBalanceTxRequest) (*rpcpb.IncreaseL1ValidatorBalanceTxResponse, error) {
	zap.L().Debug("received IncreaseL1ValidatorBalanceTx request")

	validationID, err := ids.ToID(req.ValidationId)
	if err != nil {
		return nil, fmt.Errorf("invalid validation_id: %w", err)
	}

	// Build the unsigned transaction
	utx := &txs.IncreaseL1ValidatorBalanceTx{
		BaseTx:       buildBaseTx(req.NetworkId, req.BlockchainId, req.Outputs, req.Inputs, req.Memo),
		ValidationID: validationID,
		Balance:      req.Balance,
	}

	// Serialize the transaction
	signedTx, err := txs.NewSigned(utx, txs.Codec, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create signed tx: %w", err)
	}

	expectedBytes := signedTx.Bytes()
	txID := signedTx.ID()

	resp := &rpcpb.IncreaseL1ValidatorBalanceTxResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    txID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedTx, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) DisableL1ValidatorTx(ctx context.Context, req *rpcpb.DisableL1ValidatorTxRequest) (*rpcpb.DisableL1ValidatorTxResponse, error) {
	zap.L().Debug("received DisableL1ValidatorTx request")

	validationID, err := ids.ToID(req.ValidationId)
	if err != nil {
		return nil, fmt.Errorf("invalid validation_id: %w", err)
	}

	// Build the unsigned transaction
	utx := &txs.DisableL1ValidatorTx{
		BaseTx:       buildBaseTx(req.NetworkId, req.BlockchainId, req.Outputs, req.Inputs, req.Memo),
		ValidationID: validationID,
		DisableAuth:  &secp256k1fx.Input{},
	}

	// Serialize the transaction
	signedTx, err := txs.NewSigned(utx, txs.Codec, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create signed tx: %w", err)
	}

	expectedBytes := signedTx.Bytes()
	txID := signedTx.ID()

	resp := &rpcpb.DisableL1ValidatorTxResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    txID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedTx, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

// ============================================================================
// Warp Message Handlers
// ============================================================================

func (s *server) WarpUnsignedMessage(ctx context.Context, req *rpcpb.WarpUnsignedMessageRequest) (*rpcpb.WarpUnsignedMessageResponse, error) {
	zap.L().Debug("received WarpUnsignedMessage request")

	sourceChainID, err := ids.ToID(req.SourceChainId)
	if err != nil {
		return nil, fmt.Errorf("invalid source_chain_id: %w", err)
	}

	// Build unsigned warp message
	unsignedMsg, err := warp.NewUnsignedMessage(req.NetworkId, sourceChainID, req.Payload)
	if err != nil {
		return nil, fmt.Errorf("failed to create unsigned message: %w", err)
	}

	expectedBytes := unsignedMsg.Bytes()
	msgID := unsignedMsg.ID()

	resp := &rpcpb.WarpUnsignedMessageResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    msgID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedMsg, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) WarpMessage(ctx context.Context, req *rpcpb.WarpMessageRequest) (*rpcpb.WarpMessageResponse, error) {
	zap.L().Debug("received WarpMessage request")

	// Parse the unsigned message
	unsignedMsg, err := warp.ParseUnsignedMessage(req.UnsignedMessage)
	if err != nil {
		return nil, fmt.Errorf("failed to parse unsigned message: %w", err)
	}

	// Build signed warp message
	msg, err := warp.NewMessage(unsignedMsg, &warp.BitSetSignature{
		Signers:   req.SignatureBitSet,
		Signature: *(*[bls.SignatureLen]byte)(req.Signature),
	})
	if err != nil {
		return nil, fmt.Errorf("failed to create warp message: %w", err)
	}

	expectedBytes := msg.Bytes()
	msgID := hashing.ComputeHash256Array(expectedBytes)

	resp := &rpcpb.WarpMessageResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    msgID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedMsg, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

// ============================================================================
// Warp Payload Handlers
// ============================================================================

func (s *server) RegisterL1ValidatorPayload(ctx context.Context, req *rpcpb.RegisterL1ValidatorPayloadRequest) (*rpcpb.RegisterL1ValidatorPayloadResponse, error) {
	zap.L().Debug("received RegisterL1ValidatorPayload request")

	subnetID, err := ids.ToID(req.SubnetId)
	if err != nil {
		return nil, fmt.Errorf("invalid subnet_id: %w", err)
	}

	nodeID, err := ids.ToNodeID(req.NodeId)
	if err != nil {
		return nil, fmt.Errorf("invalid node_id: %w", err)
	}

	var blsPubKey [bls.PublicKeyLen]byte
	copy(blsPubKey[:], req.BlsPublicKey)

	// Build the payload using the message package
	payload, err := message.NewRegisterL1Validator(
		subnetID,
		nodeID,
		blsPubKey,
		req.Expiry,
		convertToMsgPChainOwner(req.RemainingBalanceOwner),
		convertToMsgPChainOwner(req.DisableOwner),
		req.Weight,
	)
	if err != nil {
		return nil, fmt.Errorf("failed to create payload: %w", err)
	}

	expectedBytes := payload.Bytes()
	payloadID := hashing.ComputeHash256Array(expectedBytes)

	resp := &rpcpb.RegisterL1ValidatorPayloadResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    payloadID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedPayload, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) L1ValidatorWeightPayload(ctx context.Context, req *rpcpb.L1ValidatorWeightPayloadRequest) (*rpcpb.L1ValidatorWeightPayloadResponse, error) {
	zap.L().Debug("received L1ValidatorWeightPayload request")

	validationID, err := ids.ToID(req.ValidationId)
	if err != nil {
		return nil, fmt.Errorf("invalid validation_id: %w", err)
	}

	// Build the payload using the message package
	payload, err := message.NewL1ValidatorWeight(
		validationID,
		req.Nonce,
		req.Weight,
	)
	if err != nil {
		return nil, fmt.Errorf("failed to create payload: %w", err)
	}

	expectedBytes := payload.Bytes()
	payloadID := hashing.ComputeHash256Array(expectedBytes)

	resp := &rpcpb.L1ValidatorWeightPayloadResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    payloadID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedPayload, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) SubnetToL1ConversionPayload(ctx context.Context, req *rpcpb.SubnetToL1ConversionPayloadRequest) (*rpcpb.SubnetToL1ConversionPayloadResponse, error) {
	zap.L().Debug("received SubnetToL1ConversionPayload request")

	subnetID, err := ids.ToID(req.SubnetId)
	if err != nil {
		return nil, fmt.Errorf("invalid subnet_id: %w", err)
	}

	// Build the payload using the message package
	payload, err := message.NewSubnetToL1Conversion(subnetID)
	if err != nil {
		return nil, fmt.Errorf("failed to create payload: %w", err)
	}

	expectedBytes := payload.Bytes()
	payloadID := hashing.ComputeHash256Array(expectedBytes)

	resp := &rpcpb.SubnetToL1ConversionPayloadResponse{
		ExpectedBytes: expectedBytes,
		ComputedId:    payloadID[:],
		Success:       true,
	}

	if !bytes.Equal(req.SerializedPayload, expectedBytes) {
		resp.Message = fmt.Sprintf("serialization mismatch: expected 0x%x", expectedBytes)
		resp.Success = false
	}

	return resp, nil
}

// ============================================================================
// Helper Functions
// ============================================================================

func buildBaseTx(networkID uint32, blockchainID []byte, outputs []*rpcpb.TransferableOutput, inputs []*rpcpb.TransferableInput, memo []byte) txs.BaseTx {
	bcID := ids.ID{}
	copy(bcID[:], blockchainID)

	outs := make([]*avax.TransferableOutput, len(outputs))
	for i, o := range outputs {
		assetID := ids.ID{}
		copy(assetID[:], o.AssetId)

		addrs := make([]ids.ShortID, len(o.Output.Addresses))
		for j, a := range o.Output.Addresses {
			copy(addrs[j][:], a)
		}

		outs[i] = &avax.TransferableOutput{
			Asset: avax.Asset{ID: assetID},
			Out: &secp256k1fx.TransferOutput{
				Amt: o.Output.Amount,
				OutputOwners: secp256k1fx.OutputOwners{
					Locktime:  o.Output.Locktime,
					Threshold: o.Output.Threshold,
					Addrs:     addrs,
				},
			},
		}
	}

	ins := make([]*avax.TransferableInput, len(inputs))
	for i, in := range inputs {
		txID := ids.ID{}
		copy(txID[:], in.TxId)

		assetID := ids.ID{}
		copy(assetID[:], in.AssetId)

		ins[i] = &avax.TransferableInput{
			UTXOID: avax.UTXOID{
				TxID:        txID,
				OutputIndex: in.OutputIndex,
			},
			Asset: avax.Asset{ID: assetID},
			In: &secp256k1fx.TransferInput{
				Amt: in.Input.Amount,
				Input: secp256k1fx.Input{
					SigIndices: in.Input.SigIndices,
				},
			},
		}
	}

	return txs.BaseTx{
		BaseTx: avax.BaseTx{
			NetworkID:    networkID,
			BlockchainID: bcID,
			Outs:         outs,
			Ins:          ins,
			Memo:         memo,
		},
	}
}

// convertToPChainOwner converts proto PChainOwner to message.PChainOwner (for txs)
func convertToPChainOwner(owner *rpcpb.PChainOwner) message.PChainOwner {
	if owner == nil {
		return message.PChainOwner{}
	}

	addrs := make([]ids.ShortID, len(owner.Addresses))
	for i, a := range owner.Addresses {
		copy(addrs[i][:], a)
	}

	return message.PChainOwner{
		Threshold: owner.Threshold,
		Addresses: addrs,
	}
}

// convertToMsgPChainOwner converts proto PChainOwner to message.PChainOwner (for payloads)
func convertToMsgPChainOwner(owner *rpcpb.PChainOwner) message.PChainOwner {
	if owner == nil {
		return message.PChainOwner{}
	}

	addrs := make([]ids.ShortID, len(owner.Addresses))
	for i, a := range owner.Addresses {
		copy(addrs[i][:], a)
	}

	return message.PChainOwner{
		Threshold: owner.Threshold,
		Addresses: addrs,
	}
}

// buildWarpMessageBytes builds a warp message and returns its serialized bytes
func buildWarpMessageBytes(msg *rpcpb.WarpMessageBytes) ([]byte, error) {
	if msg == nil {
		return nil, fmt.Errorf("warp message is nil")
	}

	unsignedMsg, err := warp.ParseUnsignedMessage(msg.UnsignedMessage)
	if err != nil {
		return nil, fmt.Errorf("failed to parse unsigned message: %w", err)
	}

	warpMsg, err := warp.NewMessage(unsignedMsg, &warp.BitSetSignature{
		Signers:   msg.SignatureBitSet,
		Signature: *(*[bls.SignatureLen]byte)(msg.Signature),
	})
	if err != nil {
		return nil, fmt.Errorf("failed to create warp message: %w", err)
	}

	return warpMsg.Bytes(), nil
}
