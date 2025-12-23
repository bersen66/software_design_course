package main

import (
	"fmt"
	"os"

	"github.com/bersen66/software_design_course/chat/internal/server"
)

func main() {
	if err := server.StartServer(); err != nil {
		fmt.Fprintln(os.Stderr, "server error:", err)
		os.Exit(1)
	}
}
