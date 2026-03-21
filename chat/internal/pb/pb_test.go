package pb

import (
	"testing"
	"time"

	"github.com/golang/protobuf/ptypes/timestamp"
)

func TestChatMessage(t *testing.T) {
	now := time.Now()
	ts := &timestamp.Timestamp{
		Seconds: now.Unix(),
		Nanos:   int32(now.Nanosecond()),
	}

	msg := &ChatMessage{
		Id:     "123",
		Room:   "test-room",
		Sender: "test-user",
		Text:   "Hello, World!",
		Ts:     ts,
	}

	if msg.GetId() != "123" {
		t.Errorf("Expected Id to be '123', got '%s'", msg.GetId())
	}

	if msg.GetRoom() != "test-room" {
		t.Errorf("Expected Room to be 'test-room', got '%s'", msg.GetRoom())
	}

	if msg.GetSender() != "test-user" {
		t.Errorf("Expected Sender to be 'test-user', got '%s'", msg.GetSender())
	}

	if msg.GetText() != "Hello, World!" {
		t.Errorf("Expected Text to be 'Hello, World!', got '%s'", msg.GetText())
	}

	if msg.GetTs() == nil {
		t.Error("Expected Timestamp to be set, got nil")
	} else if msg.GetTs().GetSeconds() != ts.GetSeconds() {
		t.Errorf("Expected Timestamp Seconds to be %d, got %d", ts.GetSeconds(), msg.GetTs().GetSeconds())
	}
}

func TestSendRequest(t *testing.T) {
	req := &SendRequest{
		Room:   "test-room",
		Sender: "test-user",
		Text:   "Test message",
	}

	if req.GetRoom() != "test-room" {
		t.Errorf("Expected Room to be 'test-room', got '%s'", req.GetRoom())
	}

	if req.GetSender() != "test-user" {
		t.Errorf("Expected Sender to be 'test-user', got '%s'", req.GetSender())
	}

	if req.GetText() != "Test message" {
		t.Errorf("Expected Text to be 'Test message', got '%s'", req.GetText())
	}

	// Test default values
	reqEmpty := &SendRequest{}
	if reqEmpty.GetRoom() != "" {
		t.Errorf("Expected empty Room for zero-value SendRequest, got '%s'", reqEmpty.GetRoom())
	}
	if reqEmpty.GetSender() != "" {
		t.Errorf("Expected empty Sender for zero-value SendRequest, got '%s'", reqEmpty.GetSender())
	}
	if reqEmpty.GetText() != "" {
		t.Errorf("Expected empty Text for zero-value SendRequest, got '%s'", reqEmpty.GetText())
	}
}

// TestSendResponse tests the SendResponse structure
func TestSendResponse(t *testing.T) {
	resp := &SendResponse{
		Ok:    true,
		Id:    "456",
		Ts:    &timestamp.Timestamp{Seconds: time.Now().Unix()},
		Error: "some error",
	}

	if !resp.GetOk() {
		t.Error("Expected Ok to be true, got false")
	}

	if resp.GetId() != "456" {
		t.Errorf("Expected Id to be '456', got '%s'", resp.GetId())
	}

	if resp.GetError() != "some error" {
		t.Errorf("Expected Error to be 'some error', got '%s'", resp.GetError())
	}

	if resp.GetTs() == nil {
		t.Error("Expected Ts to be set, got nil")
	}

	respEmpty := &SendResponse{}
	if respEmpty.GetOk() {
		t.Error("Expected Ok to be false for zero-value SendResponse, got true")
	}
	if respEmpty.GetId() != "" {
		t.Errorf("Expected empty Id for zero-value SendResponse, got '%s'", respEmpty.GetId())
	}
	if respEmpty.GetError() != "" {
		t.Errorf("Expected empty Error for zero-value SendResponse, got '%s'", respEmpty.GetError())
	}
	if respEmpty.GetTs() != nil {
		t.Error("Expected Ts to be nil for zero-value SendResponse")
	}
}

func TestSubscribeRequest(t *testing.T) {
	resp := time.Now()
	ts := &timestamp.Timestamp{
		Seconds: resp.Unix(),
		Nanos:   int32(resp.Nanosecond()),
	}

	req := &SubscribeRequest{
		Room:    "test-room",
		SinceId: "100",
		Since:   ts,
	}

	if req.GetRoom() != "test-room" {
		t.Errorf("Expected Room to be 'test-room', got '%s'", req.GetRoom())
	}

	if req.GetSinceId() != "100" {
		t.Errorf("Expected SinceId to be '100', got '%s'", req.GetSinceId())
	}

	if req.GetSince() == nil {
		t.Error("Expected Since to be set, got nil")
	} else if req.GetSince().GetSeconds() != ts.GetSeconds() {
		t.Errorf("Expected Since Seconds to be %d, got %d", ts.GetSeconds(), req.GetSince().GetSeconds())
	}

	reqEmpty := &SubscribeRequest{}
	if reqEmpty.GetRoom() != "" {
		t.Errorf("Expected empty Room for zero-value SubscribeRequest, got '%s'", reqEmpty.GetRoom())
	}
	if reqEmpty.GetSinceId() != "" {
		t.Errorf("Expected empty SinceId for zero-value SubscribeRequest, got '%s'", reqEmpty.GetSinceId())
	}
	if reqEmpty.GetSince() != nil {
		t.Error("Expected Since to be nil for zero-value SubscribeRequest")
	}
}
