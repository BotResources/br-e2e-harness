package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/google/uuid"
)

var (
	adaID = uuid.MustParse("01938c1f-0000-7000-8000-000000000001")
	grcID = uuid.MustParse("01938c1f-0000-7000-8000-000000000002")

	engineeringID = uuid.MustParse("01938c1f-0000-7000-8000-0000000000a1")
	guildsID      = uuid.MustParse("01938c1f-0000-7000-8000-0000000000a2")
)

type kvEntry struct {
	Key   string          `json:"key"`
	Value json.RawMessage `json:"value"`
}

type directorySnapshot struct {
	Meta    kvEntry   `json:"meta"`
	Users   []kvEntry `json:"users"`
	Groups  []kvEntry `json:"groups"`
	Version int       `json:"snapshot_version"`
}

func strptr(s string) *string { return &s }

func mustEntry(key string, value any) kvEntry {
	raw, err := json.Marshal(value)
	if err != nil {
		panic(fmt.Sprintf("marshal %s: %v", key, err))
	}
	return kvEntry{Key: key, Value: raw}
}

func canonicalSnapshot() directorySnapshot {
	return directorySnapshot{
		Version: directoryMetaVersion,
		Meta:    mustEntry(metaKey, directoryMeta([]string{"users", "groups"})),
		Users: []kvEntry{
			mustEntry(
				userKVKey(adaID),
				publishedUserWithExtension(
					"ada@example.com",
					strptr("Ada"),
					strptr("Lovelace"),
					map[string]any{"nested": "value"},
				),
			),
			mustEntry(
				userKVKey(grcID),
				publishedUserCore("grace@example.com", nil, nil),
			),
		},
		Groups: []kvEntry{
			mustEntry(
				groupKVKey(engineeringID),
				publishedGroupWithExtension(
					"engineering",
					[]uuid.UUID{adaID, grcID},
					false,
				),
			),
			mustEntry(
				groupKVKey(guildsID),
				publishedGroupCore("guilds", nil),
			),
		},
	}
}

func main() {
	snapshot := canonicalSnapshot()
	out, err := json.MarshalIndent(snapshot, "", "  ")
	if err != nil {
		fmt.Fprintf(os.Stderr, "marshal snapshot: %v\n", err)
		os.Exit(1)
	}
	fmt.Println(string(out))
}
