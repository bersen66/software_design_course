package client

import (
	"context"
	"errors"
	"io"
	"log"
	"time"

	pb "github.com/bersen66/software_design_course/chat/internal/pb"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

type Client struct {
	conn *grpc.ClientConn
	cc   pb.ChatServiceClient
}

func Dial(ctx context.Context, addr string, opts ...grpc.DialOption) (*Client, error) {
	if len(opts) == 0 {
		opts = append(opts, grpc.WithTransportCredentials(insecure.NewCredentials()))
	}
	conn, err := grpc.NewClient(addr, opts...)
	if err != nil {
		return nil, err
	}
	return &Client{
		conn: conn,
		cc:   pb.NewChatServiceClient(conn),
	}, nil
}

func (c *Client) Close() error {
	if c == nil || c.conn == nil {
		return nil
	}
	return c.conn.Close()
}

func (c *Client) Send(ctx context.Context, room, sender, text string) (*pb.SendResponse, error) {
	if c == nil || c.cc == nil {
		return nil, errors.New("client not connected")
	}
	req := &pb.SendRequest{
		Room:   room,
		Sender: sender,
		Text:   text,
	}
	return c.cc.Send(ctx, req)
}

func (c *Client) SendWithTimeout(room, sender, text string, timeout time.Duration) (*pb.SendResponse, error) {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	return c.Send(ctx, room, sender, text)
}

func (c *Client) Subscribe(ctx context.Context, req *pb.SubscribeRequest) (<-chan *pb.ChatMessage, context.CancelFunc, error) {
	if c == nil || c.cc == nil {
		return nil, nil, errors.New("client not connected")
	}
	ctxSub, cancel := context.WithCancel(ctx)
	stream, err := c.cc.Notify(ctxSub, req)
	if err != nil {
		cancel()
		return nil, nil, err
	}

	out := make(chan *pb.ChatMessage, 32)
	go func() {
		defer close(out)
		for {
			m, err := stream.Recv()
			if err == io.EOF {
				return
			}
			if err != nil {
				if ctxSub.Err() != nil {
					return
				}
				log.Printf("notify recv error: %v", err)
				return
			}
			select {
			case out <- m:
			case <-ctxSub.Done():
				return
			}
		}
	}()

	return out, cancel, nil
}

func (c *Client) SubscribeRoom(ctx context.Context, room string) (<-chan *pb.ChatMessage, context.CancelFunc, error) {
	return c.Subscribe(ctx, &pb.SubscribeRequest{Room: room})
}
