package main

import "fmt"

const (
	integrationPrefix = "integration"
	cmdToken          = "cmd"
	evtToken          = "evt"
)

type commandCoords struct {
	receiver  string
	aggregate string
	verb      string
	version   uint8
}

type eventCoords struct {
	producer  string
	aggregate string
	fact      string
	version   uint8
}

func (c commandCoords) subject() string {
	return fmt.Sprintf(
		"%s.%s.%s.%s.%s.v%d",
		integrationPrefix, cmdToken, c.receiver, c.aggregate, c.verb, c.version,
	)
}

func (e eventCoords) subject() string {
	return fmt.Sprintf(
		"%s.%s.%s.%s.%s.v%d",
		integrationPrefix, evtToken, e.producer, e.aggregate, e.fact, e.version,
	)
}

var commandSamples = []commandCoords{
	{receiver: "notifier", aggregate: "notification", verb: "deliver", version: 1},
	{receiver: "identity", aggregate: "service_scope", verb: "declare", version: 1},
	{receiver: "identity", aggregate: "user", verb: "provision", version: 2},
}

var eventSamples = []eventCoords{
	{producer: "identity", aggregate: "user", fact: "created", version: 1},
	{producer: "identity", aggregate: "service_scope", fact: "accepted", version: 1},
	{producer: "identity", aggregate: "group", fact: "renamed", version: 3},
}
