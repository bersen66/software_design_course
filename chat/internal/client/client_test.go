package client

import (
	"context"
	"errors"
	"testing"
	"time"

	pb "github.com/bersen66/software_design_course/chat/internal/pb"
	"github.com/golang/protobuf/ptypes/timestamp"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
)

func TestDial(t *testing.T) {
	addr := "localhost:50051"
	ctx := context.Background()

	client, err := Dial(ctx, addr)

	if err != nil {
		if client == nil {
			t.Error("Expected client to be created even with connection error")
		}
	} else if client == nil {
		t.Error("Expected client to be created, got nil")
	}

	if client != nil {
		if client.conn == nil {
			t.Error("Expected connection to be set, got nil")
		}
		if client.cc == nil {
			t.Error("Expected ChatServiceClient to be set, got nil")
		}
	}

	client2, err := Dial(ctx, addr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil && client2 == nil {
		t.Error("Expected client to be created with custom options even with connection error")
	}
}

func TestClose(t *testing.T) {
	client := &Client{}

	err := client.Close()
	if err != nil {
		t.Errorf("Expected nil error when closing nil client, got: %v", err)
	}

	client.conn = nil
	err = client.Close()
	if err != nil {
		t.Errorf("Expected nil error when connection is nil, got: %v", err)
	}
}

type mockChatServiceClient struct {
	sendResponse  *pb.SendResponse
	sendError     error
	notifyChannel chan *pb.ChatMessage
	notifyError   error
}

func (m *mockChatServiceClient) Send(ctx context.Context, in *pb.SendRequest, opts ...grpc.CallOption) (*pb.SendResponse, error) {
	return m.sendResponse, m.sendError
}

func (m *mockChatServiceClient) Notify(ctx context.Context, in *pb.SubscribeRequest, opts ...grpc.CallOption) (pb.ChatService_NotifyClient, error) {
	if m.notifyError != nil {
		return nil, m.notifyError
	}

	mockClient := &mockNotifyClient{
		responseChan: m.notifyChannel,
		ctx:          ctx,
	}

	return mockClient, nil
}

type mockNotifyClient struct {
	responseChan <-chan *pb.ChatMessage
	ctx          context.Context
}

func (m *mockNotifyClient) Recv() (*pb.ChatMessage, error) {
	select {
	case msg, ok := <-m.responseChan:
		if !ok {
			return nil, errors.New("channel closed")
		}
		return msg, nil
	case <-m.ctx.Done():
		return nil, m.ctx.Err()
	}
}

func (m *mockNotifyClient) Header() (metadata.MD, error) {
	return nil, nil
}

func (m *mockNotifyClient) Trailer() metadata.MD {
	return nil
}

func (m *mockNotifyClient) CloseSend() error {
	return nil
}

func (m *mockNotifyClient) Context() context.Context {
	return m.ctx
}

func (m *mockNotifyClient) SendMsg(any) error {
	return nil
}

func (m *mockNotifyClient) RecvMsg(any) error {
	return nil
}

func TestSend(t *testing.T) {
	tests := []struct {
		name         string
		client       *Client
		room         string
		sender       string
		text         string
		mockResponse *pb.SendResponse
		mockError    error
		expectError  bool
	}{
		{
			name: "successful send",
			client: &Client{
				cc: &mockChatServiceClient{
					sendResponse: &pb.SendResponse{Ok: true, Id: "1"},
					sendError:    nil,
				},
			},
			room:         "test-room",
			sender:       "test-user",
			text:         "hello",
			mockResponse: &pb.SendResponse{Ok: true, Id: "1"},
			mockError:    nil,
			expectError:  false,
		},
		{
			name:        "nil client",
			client:      nil,
			room:        "test-room",
			sender:      "test-user",
			text:        "hello",
			expectError: true,
		},
		{
			name: "nil cc client",
			client: &Client{
				cc: nil,
			},
			room:        "test-room",
			sender:      "test-user",
			text:        "hello",
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.client != nil {
				if _, ok := tt.client.cc.(*mockChatServiceClient); ok {
				} else if tt.mockError != nil || tt.mockResponse != nil {
					tt.client.cc = &mockChatServiceClient{
						sendResponse: tt.mockResponse,
						sendError:    tt.mockError,
					}
				}
			}

			resp, err := tt.client.Send(context.Background(), tt.room, tt.sender, tt.text)

			if tt.expectError && err == nil {
				t.Error("Expected error, got nil")
			}
			if !tt.expectError && err != nil {
				t.Errorf("Did not expect error, got: %v", err)
			}

			if !tt.expectError && resp != nil && tt.mockResponse != nil {
				if resp.Ok != tt.mockResponse.Ok {
					t.Errorf("Expected Ok=%v, got %v", tt.mockResponse.Ok, resp.Ok)
				}
				if resp.Id != tt.mockResponse.Id {
					t.Errorf("Expected Id=%s, got %s", tt.mockResponse.Id, resp.Id)
				}
			}
		})
	}
}

func TestSendWithTimeout(t *testing.T) {
	client := &Client{
		cc: &mockChatServiceClient{
			sendResponse: &pb.SendResponse{Ok: true, Id: "1"},
			sendError:    nil,
		},
	}

	resp, err := client.SendWithTimeout("test-room", "test-user", "test-message", 5*time.Second)

	if err != nil {
		t.Errorf("Did not expect error, got: %v", err)
	}

	if resp == nil {
		t.Error("Expected response to be returned, got nil")
	} else if !resp.Ok {
		t.Error("Expected response Ok to be true")
	}

	client2 := &Client{
		cc: &mockChatServiceClient{
			sendResponse: nil,
			sendError:    context.DeadlineExceeded,
		},
	}

	resp2, err2 := client2.SendWithTimeout("test-room", "test-user", "test-message", 1*time.Millisecond)
	if err2 == nil {
		t.Error("Expected timeout error, got nil")
	} else if err2 != context.DeadlineExceeded {
		t.Errorf("Expected DeadlineExceeded error, got: %v", err2)
	}
	if resp2 != nil {
		t.Error("Expected nil response on timeout, got response")
	}
}

func TestSubscribe(t *testing.T) {
	ctx := context.Background()

	// Test with valid client
	messageChan := make(chan *pb.ChatMessage, 1)
	messageChan <- &pb.ChatMessage{
		Id:     "1",
		Room:   "test-room",
		Sender: "test-user",
		Text:   "test message",
		Ts:     &timestamp.Timestamp{Seconds: time.Now().Unix()},
	}

	client := &Client{
		cc: &mockChatServiceClient{
			notifyChannel: messageChan,
			notifyError:   nil,
		},
	}

	req := &pb.SubscribeRequest{
		Room: "test-room",
	}

	ch, cancel, err := client.Subscribe(ctx, req)
	if err != nil {
		t.Errorf("Did not expect error, got: %v", err)
	}
	defer cancel()

	if ch == nil {
		t.Error("Expected channel to be returned, got nil")
	}

	clientNil := &Client{}
	chNil, cancelNil, errNil := clientNil.Subscribe(ctx, req)
	if errNil == nil {
		t.Error("Expected error for nil cc, got nil")
	}
	if chNil != nil {
		t.Error("Expected nil channel for error case")
	}
	if cancelNil != nil {
		cancelNil()
	}

	var clientPtr *Client
	chPtr, cancelPtr, errPtr := clientPtr.Subscribe(ctx, req)
	if errPtr == nil {
		t.Error("Expected error for nil client pointer, got nil")
	}
	if chPtr != nil {
		t.Error("Expected nil channel for nil client")
	}
	if cancelPtr != nil {
		cancelPtr()
	}
}

func TestSubscribeRoom(t *testing.T) {
	ctx := context.Background()
	messageChan := make(chan *pb.ChatMessage, 1)
	messageChan <- &pb.ChatMessage{
		Id:     "1",
		Room:   "test-room",
		Sender: "test-user",
		Text:   "test message",
		Ts:     &timestamp.Timestamp{Seconds: time.Now().Unix()},
	}

	client := &Client{
		cc: &mockChatServiceClient{
			notifyChannel: messageChan,
			notifyError:   nil,
		},
	}

	ch, cancel, err := client.SubscribeRoom(ctx, "test-room")
	if err != nil {
		t.Errorf("Did not expect error, got: %v", err)
	}
	defer cancel()

	if ch == nil {
		t.Error("Expected channel to be returned, got nil")
	}
}
