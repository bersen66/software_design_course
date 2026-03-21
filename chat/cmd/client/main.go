package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"time"

	"github.com/bersen66/software_design_course/chat/internal/client"
	"github.com/bersen66/software_design_course/chat/internal/pb"
	"github.com/bersen66/software_design_course/chat/internal/tui"
)

func main() {
	addr := flag.String("addr", "localhost:50051", "gRPC server address")
	room := flag.String("room", "general", "chat room to join")
	user := flag.String("user", "anonymous", "username to use when sending messages")
	flag.Parse()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	cli, err := client.Dial(ctx, *addr)
	if err != nil {
		fmt.Fprintln(os.Stderr, "failed to connect to server:", err)
		cancel()
		os.Exit(1)
	}
	defer func() {
		if cli != nil {
			if err := cli.Close(); err != nil {
				fmt.Fprintln(os.Stderr, "error closing client connection:", err)
			}
		}
	}()

	subscribeReq := pb.SubscribeRequest{
		Room: *room,
	}
	incoming, stopSub, err := cli.Subscribe(context.Background(), &subscribeReq)
	if err != nil {
		fmt.Fprintln(os.Stderr, "failed to subscribe:", err)
		if cerr := cli.Close(); cerr != nil {
			fmt.Fprintln(os.Stderr, "error closing client connection:", cerr)
		}
		cancel()
		os.Exit(1)
	}
	defer stopSub()

	sendFn := func(r, u, text string) error {
		// use a short timeout for sends so the UI doesn't block long
		cctx, ccancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer ccancel()
		_, err := cli.Send(cctx, r, u, text)
		return err
	}

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt)

	runErrCh := make(chan error, 1)
	go func() {
		runErrCh <- tui.RunWithIncoming(incoming, sendFn, *room, *user)
	}()

	select {
	case <-sigCh:
		stopSub()
		time.Sleep(200 * time.Millisecond)
	case err := <-runErrCh:
		if err != nil {
			fmt.Fprintln(os.Stderr, "tui error:", err)
			stopSub()
			if cerr := cli.Close(); cerr != nil {
				fmt.Fprintln(os.Stderr, "error closing client connection:", cerr)
			} else {
				cli = nil
			}
			cancel()
			os.Exit(1)
		}
	}
}
