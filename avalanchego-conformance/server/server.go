// Copyright (C) 2019-2022, Ava Labs, Inc. All rights reserved.
// See the file LICENSE for licensing terms.

// Package server implements server.
package server

import (
	"context"
	"errors"
	"fmt"
	"net"
	"os"
	"sync"
	"time"

	"github.com/ava-labs/avalanche-rs/avalanchego-conformance/rpcpb"
	"go.uber.org/zap"
	"google.golang.org/grpc"
)

type Config struct {
	Port        uint16
	GwPort      uint16
	DialTimeout time.Duration
}

type Server interface {
	Run(rootCtx context.Context) error
}

type server struct {
	cfg Config

	rootCtx   context.Context
	closeOnce sync.Once
	closed    chan struct{}

	ln               net.Listener
	gRPCServer       *grpc.Server
	gRPCRegisterOnce sync.Once

	mu *sync.RWMutex

	rpcpb.UnimplementedPingServiceServer
	rpcpb.UnimplementedKeyServiceServer
	rpcpb.UnimplementedPackerServiceServer
	rpcpb.UnimplementedMessageServiceServer
}

var (
	ErrInvalidPort = errors.New("invalid port")
	ErrClosed      = errors.New("server closed")
)

func New(cfg Config) (Server, error) {
	if cfg.Port == 0 || cfg.GwPort == 0 {
		return nil, ErrInvalidPort
	}

	ln, err := net.Listen("tcp", fmt.Sprintf(":%d", cfg.Port))
	if err != nil {
		return nil, err
	}
	return &server{
		cfg: cfg,

		closed: make(chan struct{}),

		ln:         ln,
		gRPCServer: grpc.NewServer(),

		mu: new(sync.RWMutex),
	}, nil
}

func (s *server) Run(rootCtx context.Context) (err error) {
	s.rootCtx = rootCtx
	s.gRPCRegisterOnce.Do(func() {
		rpcpb.RegisterPingServiceServer(s.gRPCServer, s)
		rpcpb.RegisterKeyServiceServer(s.gRPCServer, s)
		rpcpb.RegisterPackerServiceServer(s.gRPCServer, s)
		rpcpb.RegisterMessageServiceServer(s.gRPCServer, s)
	})

	gRPCErrc := make(chan error)
	go func() {
		zap.L().Info("serving gRPC server", zap.Uint16("port", s.cfg.Port))
		gRPCErrc <- s.gRPCServer.Serve(s.ln)
	}()

	select {
	case <-rootCtx.Done():
		zap.L().Warn("root context is done")

		s.gRPCServer.Stop()
		zap.L().Warn("closed gRPC server")
		<-gRPCErrc

	case err = <-gRPCErrc:
		zap.L().Warn("gRPC server failed", zap.Error(err))
	}

	s.closeOnce.Do(func() {
		close(s.closed)
	})
	return err
}

func (s *server) PingService(ctx context.Context, req *rpcpb.PingServiceRequest) (*rpcpb.PingServiceResponse, error) {
	zap.L().Debug("received PingService request")
	return &rpcpb.PingServiceResponse{Pid: int32(os.Getpid())}, nil
}
