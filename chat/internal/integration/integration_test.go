package integration

import (
	"context"
	"fmt"
	"net"
	"sync"
	"testing"
	"time"

	pb "github.com/bersen66/software_design_course/chat/internal/pb"
	"github.com/bersen66/software_design_course/chat/internal/server"
	"github.com/bersen66/software_design_course/chat/internal/testutils"
	"google.golang.org/grpc"
)

const testPort = ":50052"

func TestGRPCIntegration(t *testing.T) {
	db, err := testutils.CreateTestDB()
	if err != nil {
		t.Fatalf("Failed to create test DB: %v", err)
	}
	defer db.Close()

	srv := server.NewServer(db)
	grpcServer := grpc.NewServer()
	pb.RegisterChatServiceServer(grpcServer, srv)

	lis, err := net.Listen("tcp", ":0")
	if err != nil {
		db.Close()
		t.Fatalf("Failed to listen: %v", err)
	}

	go func() {
		_ = grpcServer.Serve(lis)
	}()

	port := lis.Addr().(*net.TCPAddr).Port
	addr := fmt.Sprintf("localhost:%d", port)

	time.Sleep(100 * time.Millisecond)

	conn, err := testutils.CreateGenericTestClient(addr)
	if err != nil {
		grpcServer.GracefulStop()
		t.Fatalf("Failed to create test client: %v", err)
	}
	defer conn.Close()

	client := pb.NewChatServiceClient(conn)

	t.Run("SendAndReceiveMessage", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()

		roomName := "test-room-integration"
		senderName := "integration-tester"
		messageText := "Hello from integration test!"

		sendReq := &pb.SendRequest{
			Room:   roomName,
			Sender: senderName,
			Text:   messageText,
		}

		sendResp, err := client.Send(ctx, sendReq)
		if err != nil {
			t.Fatalf("Failed to send message: %v", err)
		}

		if !sendResp.Ok {
			t.Fatalf("Send response not OK: %s", sendResp.Error)
		}

		if sendResp.Id == "" {
			t.Error("Expected message ID to be returned")
		}

		if sendResp.Ts == nil {
			t.Error("Expected timestamp to be returned")
		}
	})

	t.Run("SubscribeAndReceiveMessages", func(t *testing.T) {
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()

		roomName := "subscription-test"
		senderName := "subscriber-tester"

		for i := range 3 {
			sendReq := &pb.SendRequest{
				Room:   roomName,
				Sender: senderName,
				Text:   fmt.Sprintf("Message %d", i+1),
			}

			ctxSend, cancelSend := context.WithTimeout(context.Background(), 2*time.Second)
			sendResp, err := client.Send(ctxSend, sendReq)
			cancelSend()
			if err != nil {
				t.Fatalf("Failed to send message %d: %v", i+1, err)
			}

			if !sendResp.Ok {
				t.Fatalf("Send response not OK for message %d: %s", i+1, sendResp.Error)
			}
		}

		subscribeReq := &pb.SubscribeRequest{
			Room: roomName,
		}

		stream, err := client.Notify(ctx, subscribeReq)
		if err != nil {
			t.Fatalf("Failed to subscribe: %v", err)
		}

		receivedMessages := 0
		timeout := time.After(5 * time.Second)

		for receivedMessages < 3 {
			select {
			case <-timeout:
				t.Fatalf("Timed out waiting for messages, only received %d", receivedMessages)
			default:
				ctxRecv, cancelRecv := context.WithTimeout(context.Background(), 1*time.Second)
				defer cancelRecv()

				if ctxRecv.Err() != nil {
					t.Log("Context error occurred")
				}

				msg, err := stream.Recv()
				if err != nil {
					t.Logf("Stream receive error: %v", err)
					break
				}

				if msg.Room != roomName {
					t.Errorf("Expected room %s, got %s", roomName, msg.Room)
				}

				if msg.Sender != senderName {
					t.Errorf("Expected sender %s, got %s", senderName, msg.Sender)
				}

				receivedMessages++
				t.Logf("Received message %d: %s", receivedMessages, msg.Text)
			}
		}

		if receivedMessages < 3 {
			t.Errorf("Expected to receive at least 3 messages, got %d", receivedMessages)
		}
	})

	t.Run("MultipleClientsBroadcast", func(t *testing.T) {
		ctx1, cancel1 := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel1()

		ctx2, cancel2 := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel2()

		roomName := "broadcast-test"
		sender1 := "client1"
		_ = "client2"

		sub1, err := client.Notify(ctx1, &pb.SubscribeRequest{Room: roomName})
		if err != nil {
			t.Fatalf("Client 1 failed to subscribe: %v", err)
		}

		sub2, err := client.Notify(ctx2, &pb.SubscribeRequest{Room: roomName})
		if err != nil {
			t.Fatalf("Client 2 failed to subscribe: %v", err)
		}

		time.Sleep(100 * time.Millisecond)

		sendReq := &pb.SendRequest{
			Room:   roomName,
			Sender: sender1,
			Text:   "Broadcast message from client1",
		}

		sendResp, err := client.Send(context.Background(), sendReq)
		if err != nil {
			t.Fatalf("Failed to send broadcast message: %v", err)
		}

		if !sendResp.Ok {
			t.Fatalf("Send response not OK: %s", sendResp.Error)
		}

		wg := sync.WaitGroup{}
		wg.Add(2)

		go func() {
			defer wg.Done()
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()

			done := make(chan struct{})
			go func() {
				defer close(done)
				msg, err := sub1.Recv()
				if err != nil {
					t.Errorf("Client 1 failed to receive broadcast: %v", err)
					return
				}

				if msg.Text != "Broadcast message from client1" {
					t.Errorf("Client 1 received unexpected message: %s", msg.Text)
				}
			}()

			select {
			case <-done:
			case <-ctx.Done():
				t.Error("Client 1 receive timed out")
			}
		}()

		go func() {
			defer wg.Done()
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()

			done := make(chan struct{})
			go func() {
				defer close(done)
				msg, err := sub2.Recv()
				if err != nil {
					t.Errorf("Client 2 failed to receive broadcast: %v", err)
					return
				}

				if msg.Text != "Broadcast message from client1" {
					t.Errorf("Client 2 received unexpected message: %s", msg.Text)
				}
			}()

			select {
			case <-done:
				// Success
			case <-ctx.Done():
				t.Error("Client 2 receive timed out")
			}
		}()

		done := make(chan struct{})
		go func() {
			wg.Wait()
			close(done)
		}()

		select {
		case <-done:
		case <-time.After(5 * time.Second):
			t.Fatal("Timed out waiting for broadcast messages to be received")
		}
	})
}

func TestDBIntegration(t *testing.T) {
	db, err := testutils.CreateTestDB()
	if err != nil {
		t.Fatalf("Failed to create test DB: %v", err)
	}
	defer db.Close()

	srv := server.NewServer(db)

	ctx := context.Background()
	sendReq := &pb.SendRequest{
		Room:   "db-test-room",
		Sender: "db-tester",
		Text:   "Test message for DB",
	}

	resp, err := srv.Send(ctx, sendReq)
	if err != nil {
		t.Fatalf("Failed to send message: %v", err)
	}

	if !resp.Ok {
		t.Fatalf("Send response not OK: %s", resp.Error)
	}

	rows, err := db.Query("SELECT id, room, sender, text FROM messages WHERE room = ?", "db-test-room")
	if err != nil {
		t.Fatalf("Failed to query DB: %v", err)
	}
	defer rows.Close()

	count := 0
	for rows.Next() {
		var id int64
		var room, sender, text string
		if err := rows.Scan(&id, &room, &sender, &text); err != nil {
			t.Fatalf("Failed to scan row: %v", err)
		}
		count++

		if room != "db-test-room" {
			t.Errorf("Expected room 'db-test-room', got '%s'", room)
		}
		if sender != "db-tester" {
			t.Errorf("Expected sender 'db-tester', got '%s'", sender)
		}
		if text != "Test message for DB" {
			t.Errorf("Expected text 'Test message for DB', got '%s'", text)
		}
	}

	if count == 0 {
		t.Error("Expected message to be saved in DB, but none found")
	}

	if err := rows.Err(); err != nil {
		t.Fatalf("Error iterating rows: %v", err)
	}
}

func tearDown(grpcServer *grpc.Server, lis net.Listener) {
	grpcServer.GracefulStop()
	if lis != nil {
		lis.Close()
	}
}
