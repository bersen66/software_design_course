package server

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"log"
	"net"
	"os"
	"strconv"
	"sync"
	"time"

	_ "modernc.org/sqlite"

	pb "github.com/bersen66/software_design_course/chat/internal/pb"
	timestamp "github.com/golang/protobuf/ptypes/timestamp"
	"google.golang.org/grpc"
)

type server struct {
	pb.UnimplementedChatServiceServer

	mu    sync.RWMutex
	rooms map[string][]*pb.ChatMessage
	subs  map[string][]chan *pb.ChatMessage
	db    *sql.DB
}

func NewServer(db *sql.DB) *server {
	return &server{
		db:    db,
		rooms: make(map[string][]*pb.ChatMessage),
		subs:  make(map[string][]chan *pb.ChatMessage),
	}
}

func (s *server) Send(ctx context.Context, req *pb.SendRequest) (*pb.SendResponse, error) {
	if req == nil {
		return &pb.SendResponse{Ok: false, Error: "nil request"}, errors.New("nil request")
	}
	if req.Room == "" {
		return &pb.SendResponse{Ok: false, Error: "room required"}, nil
	}
	sender := req.Sender
	if sender == "" {
		sender = "anonymous"
	}

	now := time.Now()
	msg := &pb.ChatMessage{
		Room:   req.Room,
		Sender: sender,
		Text:   req.Text,
		Ts: &timestamp.Timestamp{
			Seconds: now.Unix(),
			Nanos:   int32(now.Nanosecond()),
		},
	}

	if s.db != nil {
		res, err := s.db.Exec(`INSERT INTO messages (room, sender, text, ts_seconds, ts_nanos) VALUES (?, ?, ?, ?, ?)`,
			msg.Room, msg.Sender, msg.Text, msg.Ts.Seconds, msg.Ts.Nanos)
		if err != nil {
			log.Printf("saving message to db: %v", err)
		} else {
			if id, err := res.LastInsertId(); err == nil {
				msg.Id = strconv.FormatInt(id, 10)
			} else {
				log.Printf("lastinsert id: %v", err)
			}
		}
	}

	s.mu.RLock()
	subs := append([]chan *pb.ChatMessage(nil), s.subs[req.Room]...)
	s.mu.RUnlock()

	for _, ch := range subs {
		select {
		case ch <- msg:
		default:
		}
	}

	return &pb.SendResponse{
		Ok: true,
		Id: msg.Id,
		Ts: msg.Ts,
	}, nil
}

func (s *server) Notify(req *pb.SubscribeRequest, stream pb.ChatService_NotifyServer) error {
	if req == nil || req.Room == "" {
		return errors.New("room required")
	}

	ch := make(chan *pb.ChatMessage, 16)

	s.mu.Lock()
	s.subs[req.Room] = append(s.subs[req.Room], ch)
	s.mu.Unlock()

	var past []*pb.ChatMessage
	if s.db != nil {
		rows, err := s.db.Query(`SELECT id, sender, text, ts_seconds, ts_nanos FROM messages WHERE room = ? ORDER BY ts_seconds, ts_nanos, id ASC`, req.Room)
		if err != nil {
			log.Printf("query history: %v", err)
		} else {
			defer func() {
				if err := rows.Close(); err != nil {
					log.Printf("closing rows: %v", err)
				}
			}()
			for rows.Next() {
				var id int64
				var sender, text string
				var tsSec int64
				var tsNanos int64
				if err := rows.Scan(&id, &sender, &text, &tsSec, &tsNanos); err != nil {
					log.Printf("scan message: %v", err)
					continue
				}
				m := &pb.ChatMessage{
					Id:     strconv.FormatInt(id, 10),
					Room:   req.Room,
					Sender: sender,
					Text:   text,
					Ts: &timestamp.Timestamp{
						Seconds: tsSec,
						Nanos:   int32(tsNanos),
					},
				}
				past = append(past, m)
			}
			if err := rows.Err(); err != nil {
				log.Printf("rows err: %v", err)
			}
		}
	} else {
		s.mu.RLock()
		past = append([]*pb.ChatMessage(nil), s.rooms[req.Room]...)
		s.mu.RUnlock()
	}

	remove := func() {
		s.mu.Lock()
		defer s.mu.Unlock()
		list := s.subs[req.Room]
		for i, c := range list {
			if c == ch {
				// remove
				list = append(list[:i], list[i+1:]...)
				break
			}
		}
		if len(list) == 0 {
			delete(s.subs, req.Room)
		} else {
			s.subs[req.Room] = list
		}
	}

	start := 0
	if req.SinceId != "" {
		for i, m := range past {
			if m.Id == req.SinceId {
				start = i + 1
				break
			}
		}
	}

	for _, m := range past[start:] {
		if req.Since != nil {
			if m.Ts == nil {
				continue
			}
			mTime := time.Unix(m.Ts.Seconds, int64(m.Ts.Nanos))
			st := time.Unix(req.Since.Seconds, int64(req.Since.Nanos))
			if !mTime.After(st) {
				continue
			}
		}
		if err := stream.Send(m); err != nil {
			remove()
			return err
		}
	}
	for {
		select {
		case <-stream.Context().Done():
			remove()
			return nil
		case msg, ok := <-ch:
			if !ok {
				remove()
				return nil
			}
			if err := stream.Send(msg); err != nil {
				remove()
				return err
			}
		}
	}
}

func StartServer() error {
	port := os.Getenv("PORT")
	if port == "" {
		port = "50051"
	}
	dbPath := os.Getenv("CHAT_DB")
	if dbPath == "" {
		dbPath = "chat.db"
	}
	dsn := "file:" + dbPath + "?cache=shared&mode=rwc"
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return fmt.Errorf("open db: %w", err)
	}
	if err := db.Ping(); err != nil {
		return fmt.Errorf("ping db: %w", err)
	}
	_, err = db.Exec(`CREATE TABLE IF NOT EXISTS messages (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		room TEXT NOT NULL,
		sender TEXT NOT NULL,
		text TEXT NOT NULL,
		ts_seconds INTEGER NOT NULL,
		ts_nanos INTEGER NOT NULL
	)`)
	if err != nil {
		return fmt.Errorf("create table: %w", err)
	}

	addr := ":" + port
	lis, err := net.Listen("tcp", addr)
	if err != nil {
		return fmt.Errorf("listen: %w", err)
	}
	grpcS := grpc.NewServer()
	srv := NewServer(db)
	pb.RegisterChatServiceServer(grpcS, srv)
	log.Printf("gRPC server listening on %s, DB at %s", addr, dbPath)
	return grpcS.Serve(lis)
}
