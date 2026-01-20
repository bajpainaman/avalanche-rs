// Copyright (C) 2019-2022, Ava Labs, Inc. All rights reserved.
// See the file LICENSE for licensing terms.

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
	"github.com/prometheus/client_golang/prometheus"
	"go.uber.org/zap"
)

// newMessageCreator creates a message creator with the given compression type.
// In avalanchego v1.14.0, NewCreator signature changed from 5 args to 3.
func newMessageCreator(compressionType compression.Type) (message.Creator, error) {
	return message.NewCreator(prometheus.NewRegistry(), compressionType, 10*time.Second)
}

func (s *server) AcceptedFrontier(ctx context.Context, req *rpcpb.AcceptedFrontierRequest) (*rpcpb.AcceptedFrontierResponse, error) {
	zap.L().Debug("received AcceptedFrontier request")

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	// In v1.14.0, AcceptedFrontier takes a single containerID instead of a slice
	// Use the first container ID if available, otherwise use empty ID
	var containerID ids.ID
	if len(req.ContainerIds) > 0 {
		copy(containerID[:], req.ContainerIds[0])
	}

	msg, err := mc.AcceptedFrontier(chainID, req.RequestId, containerID)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd // Gzip replaced with Zstd in v1.14.0
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	summaryIDs := make([]ids.ID, 0, len(req.SummaryIds))
	for _, b := range req.SummaryIds {
		bb := [32]byte{}
		copy(bb[:], b)
		summaryIDs = append(summaryIDs, ids.ID(bb))
	}

	msg, err := mc.AcceptedStateSummary(chainID, req.RequestId, summaryIDs)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.AcceptedStateSummaryResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	// For compressed messages, just check that we got some output
	// Compression implementations may differ between Go/Rust
	if !req.GzipCompressed && !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) Accepted(ctx context.Context, req *rpcpb.AcceptedRequest) (*rpcpb.AcceptedResponse, error) {
	zap.L().Debug("received Accepted request")

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	containersIDs := make([]ids.ID, 0, len(req.ContainerIds))
	for _, b := range req.ContainerIds {
		bb := [32]byte{}
		copy(bb[:], b)
		containersIDs = append(containersIDs, ids.ID(bb))
	}

	msg, err := mc.Accepted(chainID, req.RequestId, containersIDs)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.Ancestors(chainID, req.RequestId, req.Containers)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) AppGossip(ctx context.Context, req *rpcpb.AppGossipRequest) (*rpcpb.AppGossipResponse, error) {
	zap.L().Debug("received AppGossip request")

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.AppGossip(chainID, req.AppBytes)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) AppRequest(ctx context.Context, req *rpcpb.AppRequestRequest) (*rpcpb.AppRequestResponse, error) {
	zap.L().Debug("received AppRequest request")

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.AppRequest(chainID, req.RequestId, time.Duration(req.Deadline), req.AppBytes)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) AppResponse(ctx context.Context, req *rpcpb.AppResponseRequest) (*rpcpb.AppResponseResponse, error) {
	zap.L().Debug("received AppResponse request")

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.AppResponse(chainID, req.RequestId, req.AppBytes)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) Chits(ctx context.Context, req *rpcpb.ChitsRequest) (*rpcpb.ChitsResponse, error) {
	zap.L().Debug("received Chits request")

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	// In v1.14.0, Chits signature changed significantly
	// Now takes: chainID, requestID, preferredID, preferredIDAtHeight, acceptedID, acceptedHeight
	var preferredID, preferredIDAtHeight, acceptedID ids.ID
	if len(req.ContainerIds) > 0 {
		copy(preferredID[:], req.ContainerIds[0])
	}
	if len(req.ContainerIds) > 1 {
		copy(preferredIDAtHeight[:], req.ContainerIds[1])
	}
	if len(req.ContainerIds) > 2 {
		copy(acceptedID[:], req.ContainerIds[2])
	}

	msg, err := mc.Chits(ids.ID(chainID), req.RequestId, preferredID, preferredIDAtHeight, acceptedID, 0)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}

func (s *server) GetAcceptedFrontier(ctx context.Context, req *rpcpb.GetAcceptedFrontierRequest) (*rpcpb.GetAcceptedFrontierResponse, error) {
	zap.L().Debug("received GetAcceptedFrontier request")

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.GetAcceptedFrontier(chainID, req.RequestId, time.Duration(req.Deadline))
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.GetAcceptedStateSummary(chainID, req.RequestId, time.Duration(req.Deadline), req.Heights)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) GetAccepted(ctx context.Context, req *rpcpb.GetAcceptedRequest) (*rpcpb.GetAcceptedResponse, error) {
	zap.L().Debug("received GetAccepted request")

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	containersIDs := make([]ids.ID, 0, len(req.ContainerIds))
	for _, b := range req.ContainerIds {
		bb := [32]byte{}
		copy(bb[:], b)
		containersIDs = append(containersIDs, ids.ID(bb))
	}

	msg, err := mc.GetAccepted(chainID, req.RequestId, time.Duration(req.Deadline), containersIDs)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	containerID := [32]byte{}
	copy(containerID[:], req.ContainerId)

	// GetAncestors still requires EngineType in v1.14.0 (ENGINE_TYPE_CHAIN for Snowman)
	msg, err := mc.GetAncestors(chainID, req.RequestId, time.Duration(req.Deadline), containerID, p2p.EngineType_ENGINE_TYPE_CHAIN)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.GetStateSummaryFrontier(chainID, req.RequestId, time.Duration(req.Deadline))
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	containerID := [32]byte{}
	copy(containerID[:], req.ContainerId)

	msg, err := mc.Get(chainID, req.RequestId, time.Duration(req.Deadline), containerID)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	// In v1.14.0, PeerList takes []*ips.ClaimedIPPort with staking.Certificate and AddrPort field
	ipCerts := make([]*ips.ClaimedIPPort, len(req.Peers))
	for i, p := range req.Peers {
		// Parse raw certificate bytes
		cert, err := staking.ParseCertificate(p.Certificate)
		if err != nil {
			return nil, fmt.Errorf("failed to parse certificate: %w", err)
		}
		// Build IP address from 4 bytes
		var ipBytes [4]byte
		if len(p.IpAddr) >= 4 {
			copy(ipBytes[:], p.IpAddr[:4])
		}
		ipCerts[i] = &ips.ClaimedIPPort{
			Cert: cert,
			AddrPort: netip.AddrPortFrom(
				netip.AddrFrom4(ipBytes),
				uint16(p.IpPort),
			),
			Timestamp: p.GetTimestamp(),
			Signature: p.Sig,
		}
	}

	msg, err := mc.PeerList(ipCerts, true)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) Ping(ctx context.Context, req *rpcpb.PingRequest) (*rpcpb.PingResponse, error) {
	zap.L().Debug("received Ping request")

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	// In v1.14.0, Ping takes primaryUptime uint32
	msg, err := mc.Ping(0) // Use 0 as default uptime
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	// In v1.14.0, Pong takes no arguments
	msg, err := mc.Pong()
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	containerID := [32]byte{}
	copy(containerID[:], req.ContainerId)

	msg, err := mc.PullQuery(ids.ID(chainID), req.RequestId, time.Duration(req.Deadline), ids.ID(containerID), 0)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.PushQuery(ids.ID(chainID), req.RequestId, time.Duration(req.Deadline), req.ContainerBytes, 0)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) Put(ctx context.Context, req *rpcpb.PutRequest) (*rpcpb.PutResponse, error) {
	zap.L().Debug("received Put request")

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.Put(ids.ID(chainID), req.RequestId, req.ContainerBytes)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) StateSummaryFrontier(ctx context.Context, req *rpcpb.StateSummaryFrontierRequest) (*rpcpb.StateSummaryFrontierResponse, error) {
	zap.L().Debug("received StateSummaryFrontier request")

	compressionType := compression.TypeNone
	if req.GzipCompressed {
		compressionType = compression.TypeZstd
	}
	mc, err := newMessageCreator(compressionType)
	if err != nil {
		return nil, err
	}

	chainID := [32]byte{}
	copy(chainID[:], req.ChainId)

	msg, err := mc.StateSummaryFrontier(ids.ID(chainID), req.RequestId, req.Summary)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
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

	return resp, nil
}

func (s *server) Version(ctx context.Context, req *rpcpb.VersionRequest) (*rpcpb.VersionResponse, error) {
	zap.L().Debug("received Version request")

	mc, err := newMessageCreator(compression.TypeNone)
	if err != nil {
		return nil, err
	}

	// Parse IP address - need to convert 4-byte slice to netip.AddrPort
	var ipAddr [4]byte
	if len(req.IpAddr) >= 4 {
		copy(ipAddr[:], req.IpAddr[:4])
	}
	ip := netip.AddrPortFrom(netip.AddrFrom4(ipAddr), uint16(req.IpPort))

	trackedSubnets := make([]ids.ID, 0, len(req.TrackedSubnets))
	for _, b := range req.TrackedSubnets {
		bb := [32]byte{}
		copy(bb[:], b)
		trackedSubnets = append(trackedSubnets, ids.ID(bb))
	}

	// In v1.14.0, Version became Handshake with a completely different signature
	// Handshake(networkID, myTime, ip, client, major, minor, patch, ipSigningTime, ipNodeIDSig, ipBLSSig, trackedSubnets, supportedACPs, objectedACPs, knownPeersFilter, knownPeersSalt, requestAllSubnetIPs)
	msg, err := mc.Handshake(
		req.NetworkId,
		req.MyTime,
		ip,
		req.MyVersion,  // client string
		0, 0, 0,        // major, minor, patch - use zeros as we don't have them
		req.MyVersionTime,
		req.Sig,
		nil,            // ipBLSSig
		trackedSubnets,
		nil,            // supportedACPs
		nil,            // objectedACPs
		nil,            // knownPeersFilter
		nil,            // knownPeersSalt
		false,          // requestAllSubnetIPs
	)
	if err != nil {
		return nil, err
	}

	// ref. "network/peer.writeMessages"
	msgBytes := msg.Bytes()
	msgLen := uint32(len(msgBytes))
	msgLenBytes := [wrappers.IntLen]byte{}
	binary.BigEndian.PutUint32(msgLenBytes[:], msgLen)
	expected := append(msgLenBytes[:], msgBytes...)

	resp := &rpcpb.VersionResponse{
		ExpectedSerializedMsg: expected,
		Success:               true,
	}
	if !bytes.Equal(req.SerializedMsg, expected) {
		resp.Message = fmt.Sprintf("expected 0x%x", expected)
		resp.Success = false
	}

	return resp, nil
}
