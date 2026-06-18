package main

import (
	"strings"
	"testing"
)

func TestCommandSubjectRendersTheFixedGrammar(t *testing.T) {
	c := commandCoords{receiver: "notifier", aggregate: "notification", verb: "deliver", version: 1}
	if got, want := c.subject(), "integration.cmd.notifier.notification.deliver.v1"; got != want {
		t.Fatalf("command subject = %s, want %s", got, want)
	}
}

func TestEventSubjectRendersTheFixedGrammar(t *testing.T) {
	e := eventCoords{producer: "identity", aggregate: "user", fact: "created", version: 2}
	if got, want := e.subject(), "integration.evt.identity.user.created.v2"; got != want {
		t.Fatalf("event subject = %s, want %s", got, want)
	}
}

func TestEverySubjectStartsWithTheFixedPrefix(t *testing.T) {
	for _, c := range commandSamples {
		if !strings.HasPrefix(c.subject(), "integration.cmd.") {
			t.Fatalf("command subject %s does not start with integration.cmd.", c.subject())
		}
	}
	for _, e := range eventSamples {
		if !strings.HasPrefix(e.subject(), "integration.evt.") {
			t.Fatalf("event subject %s does not start with integration.evt.", e.subject())
		}
	}
}

func TestNoSampleUsesTheDeadGrammar(t *testing.T) {
	for _, c := range commandSamples {
		if strings.HasPrefix(c.subject(), "identity.cmd.") {
			t.Fatalf("sample %s uses the dead grammar", c.subject())
		}
	}
	for _, e := range eventSamples {
		if strings.HasPrefix(e.subject(), "identity.evt.") {
			t.Fatalf("sample %s uses the dead grammar", e.subject())
		}
	}
}

func TestPublishedUserKeyUsesTheFrozenPrefix(t *testing.T) {
	key := userKvKey("0193a1b2-0000-7000-8000-000000000001")
	if want := "identity/users/0193a1b2-0000-7000-8000-000000000001"; key != want {
		t.Fatalf("user kv key = %s, want %s", key, want)
	}
}
