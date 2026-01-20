// Copyright (C) 2019-2022, Ava Labs, Inc. All rights reserved.
// See the file LICENSE for licensing terms.

// Package server implements the avalanchego-conformance gRPC server.
// NOTE: Updated for avalanchego v1.14.0 (Granite) API changes:
// - message.NewCreator: 5 args -> 3 args
// - compression.TypeGzip -> compression.TypeZstd
// - AcceptedFrontier: []ids.ID -> single ids.ID
// - Chits: completely new signature
// - Ping/Pong: parameters swapped
// - Many methods lost engineType parameter
// - Version method: REMOVED from Creator interface
// - PeerList: []ClaimedIPPort -> []*ClaimedIPPort with new struct fields

package server

import (
	"bytes"
	"context"
	"encoding/binary"
	"fmt"
	"net/netip"
	"time"

	"github.com/ava-labs/avalanche-rs/avalanchego-conformance/rpcpb"
	"github.com/ava-labs/avalanchego/ids"
	"github.com/ava-labs/avalanchego/message"
	"github.com/ava-labs/avalanchego/proto/pb/p2p"
	"github.com/ava-labs/avalanchego/staking"
	"github.com/ava-labs/avalanchego/utils/compression"
	"github.com/ava-labs/avalanchego/utils/ips"
	"github.com/ava-labs/avalanchego/utils/wrappers"
	"github.com/klauspost/compress/zstd"
	"github.com/prometheus/client_golang/prometheus"
	"go.uber.org/zap"
)

// createMessageCreator creates a message.Creator with the v1.14.0 API
// NewCreator signature changed from 5 args to 3 args:
// Old: (logger, registerer, namespace, compressionType, maxMessageTimeout)
// New: (registerer, compressionType, maxMessageTimeout)
func createMessageCreator(useCompression bool) (message.Creator, error) {
	compressType := compression.TypeNone
	if useCompression {
		compressType = compression.TypeZstd // v1.14.0 uses zstd instead of gzip
	}
	return message.NewCreator(
		prometheus.NewRegistry(),
		compressType,
		10*time.Second,
	)
}

// decompressZstd decompresses zstd-compressed data
func decompressZstd(data []byte) ([]byte, error) {
	decoder, err := zstd.NewReader(nil)
	if err != nil {
		return nil, err
	}
	defer decoder.Close()
	return decoder.DecodeAll(data, nil)
}

func (s *server) AcceptedFrontier(ctx context.Context, req *rpcpb.AcceptedFrontierRequest) (*rpcpb.AcceptedFrontierResponse, error) {
	zap.L().Debug("received AcceptedFrontier request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	// v1.14.0: AcceptedFrontier now takes a single containerID instead of a slice
	var containerID ids.ID
	if len(req.ContainerIds) > 0 {
		copy(containerID[:], req.ContainerIds[0])
	}

	msg, err := mc.AcceptedFrontier(chainID, req.RequestId, containerID)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.AcceptedFrontierResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) AcceptedStateSummary(ctx context.Context, req *rpcpb.AcceptedStateSummaryRequest) (*rpcpb.AcceptedStateSummaryResponse, error) {
	zap.L().Debug("received AcceptedStateSummary request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	summaryIDs := make([]ids.ID, 0, len(req.SummaryIds))
	for _, b := range req.SummaryIds {
		var id ids.ID
		copy(id[:], b)
		summaryIDs = append(summaryIDs, id)
	}

	msg, err := mc.AcceptedStateSummary(chainID, req.RequestId, summaryIDs)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.AcceptedStateSummaryResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		// v1.14.0 uses zstd compression
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) Accepted(ctx context.Context, req *rpcpb.AcceptedRequest) (*rpcpb.AcceptedResponse, error) {
	zap.L().Debug("received Accepted request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	containersIDs := make([]ids.ID, 0, len(req.ContainerIds))
	for _, b := range req.ContainerIds {
		var id ids.ID
		copy(id[:], b)
		containersIDs = append(containersIDs, id)
	}

	msg, err := mc.Accepted(chainID, req.RequestId, containersIDs)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.AcceptedResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) Ancestors(ctx context.Context, req *rpcpb.AncestorsRequest) (*rpcpb.AncestorsResponse, error) {
	zap.L().Debug("received Ancestors request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.Ancestors(chainID, req.RequestId, req.Containers)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.AncestorsResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) AppGossip(ctx context.Context, req *rpcpb.AppGossipRequest) (*rpcpb.AppGossipResponse, error) {
	zap.L().Debug("received AppGossip request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.AppGossip(chainID, req.AppBytes)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.AppGossipResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) AppRequest(ctx context.Context, req *rpcpb.AppRequestRequest) (*rpcpb.AppRequestResponse, error) {
	zap.L().Debug("received AppRequest request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.AppRequest(chainID, req.RequestId, time.Duration(req.Deadline), req.AppBytes)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.AppRequestResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) AppResponse(ctx context.Context, req *rpcpb.AppResponseRequest) (*rpcpb.AppResponseResponse, error) {
	zap.L().Debug("received AppResponse request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.AppResponse(chainID, req.RequestId, req.AppBytes)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.AppResponseResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) Chits(ctx context.Context, req *rpcpb.ChitsRequest) (*rpcpb.ChitsResponse, error) {
	zap.L().Debug("received Chits request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	// v1.14.0: Chits signature changed completely
	// Old: Chits(chainID, requestID, containerIDs []ids.ID, ???)
	// New: Chits(chainID, requestID, preferredID, preferredIDAtHeight, acceptedID ids.ID, acceptedHeight uint64)
	var preferredID, preferredIDAtHeight, acceptedID ids.ID
	if len(req.ContainerIds) > 0 {
		copy(preferredID[:], req.ContainerIds[0])
	}
	if len(req.ContainerIds) > 1 {
		copy(acceptedID[:], req.ContainerIds[1])
	}

	msg, err := mc.Chits(chainID, req.RequestId, preferredID, preferredIDAtHeight, acceptedID, 0)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.ChitsResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x (note: Chits API changed in v1.14.0)", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) GetAcceptedFrontier(ctx context.Context, req *rpcpb.GetAcceptedFrontierRequest) (*rpcpb.GetAcceptedFrontierResponse, error) {
	zap.L().Debug("received GetAcceptedFrontier request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	// v1.14.0: removed engineType parameter
	msg, err := mc.GetAcceptedFrontier(chainID, req.RequestId, time.Duration(req.Deadline))
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.GetAcceptedFrontierResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) GetAcceptedStateSummary(ctx context.Context, req *rpcpb.GetAcceptedStateSummaryRequest) (*rpcpb.GetAcceptedStateSummaryResponse, error) {
	zap.L().Debug("received GetAcceptedStateSummary request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.GetAcceptedStateSummary(chainID, req.RequestId, time.Duration(req.Deadline), req.Heights)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.GetAcceptedStateSummaryResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) GetAccepted(ctx context.Context, req *rpcpb.GetAcceptedRequest) (*rpcpb.GetAcceptedResponse, error) {
	zap.L().Debug("received GetAccepted request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	containersIDs := make([]ids.ID, 0, len(req.ContainerIds))
	for _, b := range req.ContainerIds {
		var id ids.ID
		copy(id[:], b)
		containersIDs = append(containersIDs, id)
	}

	// v1.14.0: removed engineType parameter
	msg, err := mc.GetAccepted(chainID, req.RequestId, time.Duration(req.Deadline), containersIDs)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.GetAcceptedResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) GetAncestors(ctx context.Context, req *rpcpb.GetAncestorsRequest) (*rpcpb.GetAncestorsResponse, error) {
	zap.L().Debug("received GetAncestors request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	containerID := ids.ID{}
	copy(containerID[:], req.ContainerId)

	// v1.13.5: GetAncestors still has engineType parameter (unlike v1.14.0)
	// ENGINE_TYPE_CHAIN is used for Snowman consensus (linear chain)
	msg, err := mc.GetAncestors(chainID, req.RequestId, time.Duration(req.Deadline), containerID, p2p.EngineType_ENGINE_TYPE_CHAIN)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.GetAncestorsResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) GetStateSummaryFrontier(ctx context.Context, req *rpcpb.GetStateSummaryFrontierRequest) (*rpcpb.GetStateSummaryFrontierResponse, error) {
	zap.L().Debug("received GetStateSummaryFrontier request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.GetStateSummaryFrontier(chainID, req.RequestId, time.Duration(req.Deadline))
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.GetStateSummaryFrontierResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) Get(ctx context.Context, req *rpcpb.GetRequest) (*rpcpb.GetResponse, error) {
	zap.L().Debug("received Get request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	containerID := ids.ID{}
	copy(containerID[:], req.ContainerId)

	// v1.14.0: removed engineType parameter
	msg, err := mc.Get(chainID, req.RequestId, time.Duration(req.Deadline), containerID)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.GetResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) Peerlist(ctx context.Context, req *rpcpb.PeerlistRequest) (*rpcpb.PeerlistResponse, error) {
	zap.L().Debug("received Peerlist request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	// v1.14.0: PeerList now takes []*ips.ClaimedIPPort with *staking.Certificate and AddrPort
	ipCerts := make([]*ips.ClaimedIPPort, len(req.Peers))
	for i, p := range req.Peers {
		// Parse IP address to netip.AddrPort
		var addr netip.Addr
		if len(p.IpAddr) == 4 {
			addr = netip.AddrFrom4([4]byte(p.IpAddr))
		} else if len(p.IpAddr) == 16 {
			addr = netip.AddrFrom16([16]byte(p.IpAddr))
		} else {
			return nil, fmt.Errorf("invalid IP address format: expected 4 or 16 bytes, got %d", len(p.IpAddr))
		}
		addrPort := netip.AddrPortFrom(addr, uint16(p.IpPort))

		// Parse certificate using staking package
		cert, err := staking.ParseCertificate(p.Certificate)
		if err != nil {
			return nil, fmt.Errorf("failed to parse certificate: %w", err)
		}

		ipCerts[i] = &ips.ClaimedIPPort{
			Cert:      cert,
			AddrPort:  addrPort,
			Timestamp: p.GetTimestamp(),
			Signature: p.Sig,
		}
	}

	msg, err := mc.PeerList(ipCerts, true)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.PeerlistResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) Ping(ctx context.Context, req *rpcpb.PingRequest) (*rpcpb.PingResponse, error) {
	zap.L().Debug("received Ping request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	// v1.14.0: Ping now takes primaryUptime parameter
	// Old: Ping()
	// New: Ping(primaryUptime uint32)
	msg, err := mc.Ping(0)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.PingResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) Pong(ctx context.Context, req *rpcpb.PongRequest) (*rpcpb.PongResponse, error) {
	zap.L().Debug("received Pong request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	// v1.14.0: Pong now takes no parameters
	// Old: Pong(uptimePct, subnetUptimes)
	// New: Pong()
	msg, err := mc.Pong()
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.PongResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) PullQuery(ctx context.Context, req *rpcpb.PullQueryRequest) (*rpcpb.PullQueryResponse, error) {
	zap.L().Debug("received PullQuery request")

	mc, err := createMessageCreator(false)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	containerID := ids.ID{}
	copy(containerID[:], req.ContainerId)

	// v1.14.0: PullQuery now takes requestedHeight instead of engineType
	// Old: PullQuery(chainID, requestID, deadline, containerID, engineType)
	// New: PullQuery(chainID, requestID, deadline, containerID, requestedHeight uint64)
	msg, err := mc.PullQuery(chainID, req.RequestId, time.Duration(req.Deadline), containerID, 0)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.PullQueryResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) PushQuery(ctx context.Context, req *rpcpb.PushQueryRequest) (*rpcpb.PushQueryResponse, error) {
	zap.L().Debug("received PushQuery request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	// v1.14.0: PushQuery now takes requestedHeight instead of engineType
	// Old: PushQuery(chainID, requestID, deadline, container, engineType)
	// New: PushQuery(chainID, requestID, deadline, container, requestedHeight uint64)
	msg, err := mc.PushQuery(chainID, req.RequestId, time.Duration(req.Deadline), req.ContainerBytes, 0)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.PushQueryResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) Put(ctx context.Context, req *rpcpb.PutRequest) (*rpcpb.PutResponse, error) {
	zap.L().Debug("received Put request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	// v1.14.0: removed engineType parameter
	msg, err := mc.Put(chainID, req.RequestId, req.ContainerBytes)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.PutResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) StateSummaryFrontier(ctx context.Context, req *rpcpb.StateSummaryFrontierRequest) (*rpcpb.StateSummaryFrontierResponse, error) {
	zap.L().Debug("received StateSummaryFrontier request")

	mc, err := createMessageCreator(req.GzipCompressed)
	if err != nil {
		return nil, err
	}

	chainID := ids.ID{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.StateSummaryFrontier(chainID, req.RequestId, req.Summary)
	if err != nil {
		return nil, err
	}

	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.StateSummaryFrontierResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}
	if req.GzipCompressed {
		expectedDecompressed, err := decompressZstd(expected[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress expected: %w", err)
		}
		receivedDecompressed, err := decompressZstd(req.SerializedMsg[wrappers.IntLen+2:])
		if err != nil {
			return nil, fmt.Errorf("failed to decompress received: %w", err)
		}
		if !bytes.Equal(expectedDecompressed, receivedDecompressed) {
			resp.Message = fmt.Sprintf("decompressed output expected [%x], got [%x]", expectedDecompressed, receivedDecompressed)
			resp.Success = false
		}
	}

	return resp, nil
}

func (s *server) Version(ctx context.Context, req *rpcpb.VersionRequest) (*rpcpb.VersionResponse, error) {
	zap.L().Debug("received Version request")

	// v1.14.0: The Version method has been REMOVED from the message.Creator interface
	// Version negotiation now happens via Handshake message.
	// This conformance test is no longer applicable for v1.14.0.
	resp := &rpcpb.VersionResponse{
		ExpectedSerializedMsg: nil,
		Success:               false,
		Message:               "Version message method removed in v1.14.0 - use Handshake instead",
	}

	return resp, nil
}
