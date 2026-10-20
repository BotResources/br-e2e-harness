package main

import (
	"net"
	"testing"
)

func TestTheSubjectListensOnLoopbackOnly(t *testing.T) {
	listener, err := net.Listen("tcp", listenAddr("0"))
	if err != nil {
		t.Fatalf("listen on %q: %v", listenAddr("0"), err)
	}
	defer listener.Close()
	tcp, ok := listener.Addr().(*net.TCPAddr)
	if !ok {
		t.Fatalf("listener address %v is not a TCP address", listener.Addr())
	}
	if !tcp.IP.IsLoopback() {
		t.Fatalf("the subject must listen on loopback only, got %s", tcp.IP)
	}
}

func TestTheListenAddressNamesTheLoopbackHost(t *testing.T) {
	if got, want := listenAddr("8080"), "127.0.0.1:8080"; got != want {
		t.Fatalf("listenAddr = %q, want %q", got, want)
	}
}
