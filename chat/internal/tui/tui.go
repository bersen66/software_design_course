package tui

import (
	"fmt"
	"strings"

	pb "github.com/bersen66/software_design_course/chat/internal/pb"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
)

type model struct {
	input    textinput.Model
	messages []string

	room string
	user string

	send func(room, user, text string) error
}

type incomingMsg struct {
	M *pb.ChatMessage
}

func New() *model {
	ti := textinput.New()
	ti.Placeholder = "Type a message and press Enter"
	ti.Focus()
	ti.CharLimit = 512
	ti.Width = 60

	m := &model{
		input:    ti,
		messages: make([]string, 0, 32),
	}
	return m
}

func NewWithSender(room, user string, send func(room, user, text string) error) *model {
	m := New()
	m.room = room
	m.user = user
	m.send = send
	return m
}

func (m *model) SetSender(room, user string, send func(room, user, text string) error) {
	m.room = room
	m.user = user
	m.send = send
}

func Run() error {
	p := tea.NewProgram(New())
	retModel, err := p.Run()
	if err != nil {
		return fmt.Errorf("starting TUI program: %w", err)
	}
	_ = retModel
	return nil
}

func RunWithIncoming(incoming <-chan *pb.ChatMessage, sendFn func(room, user, text string) error, room, user string) error {
	m := NewWithSender(room, user, sendFn)
	p := tea.NewProgram(m)

	if incoming != nil {
		go func() {
			for msg := range incoming {
				p.Send(incomingMsg{M: msg})
			}
		}()
	}

	retModel, err := p.Run()
	if err != nil {
		return fmt.Errorf("starting TUI program: %w", err)
	}
	_ = retModel
	return nil
}

func (m *model) Init() tea.Cmd {
	return nil
}

func (m *model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c", "esc":
			return m, tea.Quit
		case "enter":
			val := strings.TrimSpace(m.input.Value())
			if val != "" {
				if m.send != nil {
					go func(text string) {
						_ = m.send(m.room, m.user, text)
					}(val)
				} else {
					m.messages = append(m.messages, val)
				}
				m.input.SetValue("")
			}
			return m, nil
		}
	case incomingMsg:
		if msg.M != nil {
			m.messages = append(m.messages, fmt.Sprintf("%s: %s", msg.M.GetSender(), msg.M.GetText()))
		}
		return m, nil
	}

	var cmd tea.Cmd
	m.input, cmd = m.input.Update(msg)
	return m, cmd
}

func (m *model) View() string {
	var b strings.Builder
	b.WriteString(fmt.Sprintf("Room: %s  User: %s\n", m.room, m.user))
	b.WriteString("Simple Chat TUI (stub)\n")
	b.WriteString("Press Enter to send, Esc/Ctrl+C to quit\n")
	b.WriteString("----------------------------------------\n\n")

	if len(m.messages) == 0 {
		b.WriteString("(no messages yet)\n\n")
	} else {
		start := 0
		if len(m.messages) > 100 {
			start = len(m.messages) - 100
		}
		for _, msg := range m.messages[start:] {
			b.WriteString(fmt.Sprintf("• %s\n", msg))
		}
		b.WriteString("\n")
	}

	b.WriteString(m.input.View())
	b.WriteString("\n")
	return b.String()
}
