package server

import (
	"context"
	"database/sql"
	"testing"
	"time"

	pb "github.com/bersen66/software_design_course/chat/internal/pb"
	"github.com/bersen66/software_design_course/chat/internal/testutils"
	"google.golang.org/grpc/metadata"
)

type MD = metadata.MD

func TestNewServer(t *testing.T) {
	db := &sql.DB{}
	srv := NewServer(db)

	if srv.db != db {
		t.Errorf("Expected server to have the provided database, got %v", srv.db)
	}

	if srv.rooms == nil {
		t.Error("Expected rooms map to be initialized, got nil")
	}

	if srv.subs == nil {
		t.Error("Expected subs map to be initialized, got nil")
	}
}

func TestSend(t *testing.T) {
	db, err := testutils.CreateTestDB()
	if err != nil {
		t.Fatalf("Failed to create test DB: %v", err)
	}
	defer db.Close()

	server := NewServer(db)

	tests := []struct {
		name           string
		request        *pb.SendRequest
		expectedOK     bool
		expectError    bool
		expectedErrMsg string
	}{
		{
			name: "valid message",
			request: &pb.SendRequest{
				Room:   "test-room",
				Sender: "test-user",
				Text:   "hello world",
			},
			expectedOK:  true,
			expectError: false,
		},
		{
			name:           "nil request",
			request:        nil,
			expectedOK:     false,
			expectError:    true,
			expectedErrMsg: "",
		},
		{
			name: "empty room",
			request: &pb.SendRequest{
				Room:   "",
				Sender: "test-user",
				Text:   "hello world",
			},
			expectedOK:     false,
			expectError:    false,
			expectedErrMsg: "room required",
		},
		{
			name: "empty sender becomes anonymous",
			request: &pb.SendRequest{
				Room:   "test-room",
				Sender: "",
				Text:   "hello world",
			},
			expectedOK:  true,
			expectError: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			resp, err := server.Send(context.Background(), tt.request)

			if tt.expectError && err == nil {
				t.Errorf("Expected error, got nil")
			}
			if !tt.expectError && err != nil {
				t.Errorf("Did not expect error, got: %v", err)
			}

			if resp != nil && resp.Ok != tt.expectedOK {
				t.Errorf("Expected Ok=%v, got %v", tt.expectedOK, resp.Ok)
			}

			if tt.expectedErrMsg != "" && resp != nil && resp.Error != tt.expectedErrMsg {
				t.Errorf("Expected error message '%s', got '%s'", tt.expectedErrMsg, resp.Error)
			}

			if tt.expectedOK && tt.request != nil && tt.request.Room != "" {
				if resp != nil && resp.Id == "" {
					t.Error("Expected message ID to be set, got empty string")
				}

				if resp != nil && resp.Ts == nil {
					t.Error("Expected timestamp to be set, got nil")
				} else if resp != nil && resp.Ts != nil {
					now := time.Now()
					tsTime := time.Unix(resp.Ts.Seconds, int64(resp.Ts.Nanos))
					if tsTime.After(now.Add(1 * time.Minute)) {
						t.Error("Timestamp is too far in the future")
					}
				}
			}
		})
	}
}

type mockServerStream struct {
	ctx       context.Context
	send      func(*pb.ChatMessage) error
	recv      func() (*pb.ChatMessage, error)
	closeSend func() error
}

func (m *mockServerStream) Context() context.Context {
	return m.ctx
}

func (m *mockServerStream) SendMsg(any) error {
	return nil
}

func (m *mockServerStream) RecvMsg(any) error {
	return nil
}

func (m *mockServerStream) SetHeader(MD) error {
	return nil
}

func (m *mockServerStream) SendHeader(MD) error {
	return nil
}

func (m *mockServerStream) SetTrailer(MD) {
}

func (m *mockServerStream) Trailer() MD {
	return nil
}

func (m *mockServerStream) Send(response *pb.ChatMessage) error {
	if m.send != nil {
		return m.send(response)
	}
	return nil
}

func (m *mockServerStream) Method() string {
	return ""
}

func TestNotify(t *testing.T) {
	db, err := testutils.CreateTestDB()
	if err != nil {
		t.Fatalf("Failed to create test DB: %v", err)
	}
	defer db.Close()

	server := NewServer(db)

	// Test with empty room
	req := &pb.SubscribeRequest{Room: ""}
	stream := &mockServerStream{
		ctx: context.Background(),
		send: func(msg *pb.ChatMessage) error {
			return nil
		},
	}

	err = server.Notify(req, stream)
	if err == nil {
		t.Error("Expected error for empty room, got nil")
	}

	done := make(chan error, 1)
	go func() {
		testCtx, testCancel := context.WithCancel(context.Background())
		defer testCancel()

		testStream := &mockServerStream{
			ctx: testCtx,
			send: func(msg *pb.ChatMessage) error {
				return nil
			},
		}

		done <- server.Notify(&pb.SubscribeRequest{Room: "test-room"}, testStream)
	}()

	select {
	case <-done:
	case <-time.After(100 * time.Millisecond):
	}
}
