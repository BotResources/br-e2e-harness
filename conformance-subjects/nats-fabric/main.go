package main

import (
	"encoding/json"
	"log"
	"os"
)

type commandSubject struct {
	Receiver  string `json:"receiver"`
	Aggregate string `json:"aggregate"`
	Verb      string `json:"verb"`
	Version   uint8  `json:"version"`
	Subject   string `json:"subject"`
}

type eventSubject struct {
	Producer  string `json:"producer"`
	Aggregate string `json:"aggregate"`
	Fact      string `json:"fact"`
	Version   uint8  `json:"version"`
	Subject   string `json:"subject"`
}

type publishedUserEntry struct {
	Key   string            `json:"key"`
	Value publishedUserWire `json:"value"`
}

type frozenWire struct {
	CommandSubjects []commandSubject     `json:"command_subjects"`
	EventSubjects   []eventSubject       `json:"event_subjects"`
	PublishedUsers  []publishedUserEntry `json:"published_users"`
	PoisonUserKey   string               `json:"poison_user_key"`
	PoisonUserValue string               `json:"poison_user_value"`
}

func render() frozenWire {
	commands := make([]commandSubject, 0, len(commandSamples))
	for _, c := range commandSamples {
		commands = append(commands, commandSubject{
			Receiver:  c.receiver,
			Aggregate: c.aggregate,
			Verb:      c.verb,
			Version:   c.version,
			Subject:   c.subject(),
		})
	}

	events := make([]eventSubject, 0, len(eventSamples))
	for _, e := range eventSamples {
		events = append(events, eventSubject{
			Producer:  e.producer,
			Aggregate: e.aggregate,
			Fact:      e.fact,
			Version:   e.version,
			Subject:   e.subject(),
		})
	}

	users := []publishedUserEntry{
		{
			Key:   userKvKey("0193a1b2-0000-7000-8000-000000000001"),
			Value: sampleUser("ada@example.com", "Ada", "Lovelace"),
		},
		{
			Key:   userKvKey("0193a1b2-0000-7000-8000-000000000002"),
			Value: sampleUser("grace@example.com", "Grace", "Hopper"),
		},
	}

	return frozenWire{
		CommandSubjects: commands,
		EventSubjects:   events,
		PublishedUsers:  users,
		PoisonUserKey:   userKvKey("0193a1b2-0000-7000-8000-0000000000ff"),
		PoisonUserValue: poisonUserValue,
	}
}

func main() {
	out, err := json.Marshal(render())
	if err != nil {
		log.Fatalf("marshal frozen wire: %v", err)
	}
	if _, err := os.Stdout.Write(out); err != nil {
		log.Fatalf("write frozen wire: %v", err)
	}
}
