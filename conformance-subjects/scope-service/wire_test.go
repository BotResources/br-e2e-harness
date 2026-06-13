package main

import (
	"encoding/json"
	"reflect"
	"testing"
)

func TestDeclareCommandMatchesGoldenShape(t *testing.T) {
	cfg := config{
		serviceKey:     "notifier",
		labelKey:       "label.notifier",
		descriptionKey: "desc.notifier",
		scopeKeys:      []string{"notifier:read", "notifier:admin"},
		platformOnly:   false,
		enabled:        true,
	}
	cmd := integrationCommand{
		CommandID:   "0190a1b2-0000-7000-8000-000000000001",
		CommandType: commandType,
		Version:     wireVersion,
		IssuedAt:    "2023-11-14T22:13:20Z",
		Metadata: messageMetadata{
			ActorID:       declaringActorID("notifier"),
			ActorKind:     "service",
			CorrelationID: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
		},
		Payload: cfg.declaration(),
	}

	got, err := json.Marshal(cmd)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	golden := `{
      "command_id": "0190a1b2-0000-7000-8000-000000000001",
      "command_type": "service_scope.declare",
      "version": 1,
      "issued_at": "2023-11-14T22:13:20Z",
      "metadata": {
        "actor_id": "b10a8b19-5b18-53aa-b872-81dd00af0976",
        "actor_kind": "service",
        "correlation_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
      },
      "payload": {
        "declaration": {
          "manifest": {
            "key": "notifier",
            "label_key": "label.notifier",
            "description_key": "desc.notifier"
          },
          "scopes": [
            {"key": "notifier:read",  "label_key": "label.notifier", "description_key": "desc.notifier", "platform_only": false},
            {"key": "notifier:admin", "label_key": "label.notifier", "description_key": "desc.notifier", "platform_only": false}
          ]
        }
      }
    }`

	assertJSONEqual(t, got, []byte(golden))
}

func TestMetadataOmitsCausationWhenEmpty(t *testing.T) {
	m := messageMetadata{ActorID: "x", ActorKind: "service", CorrelationID: "c"}
	b, err := json.Marshal(m)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var asMap map[string]any
	if err := json.Unmarshal(b, &asMap); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if _, present := asMap["causation_id"]; present {
		t.Fatalf("causation_id must be omitted when empty, got %s", b)
	}
}

func TestDeclaringActorIDMatchesRustV5(t *testing.T) {
	// br-util-scope-declaration::declaring_actor for "notifier" under the
	// crate namespace; cross-checked against the golden JSON.
	const want = "b10a8b19-5b18-53aa-b872-81dd00af0976"
	if got := declaringActorID("notifier"); got != want {
		t.Fatalf("declaringActorID(notifier) = %s, want %s", got, want)
	}
}

func TestEmptyScopeSetSerializesAsArray(t *testing.T) {
	cfg := config{serviceKey: "notifier", scopeKeys: nil}
	b, err := json.Marshal(cfg.declaration())
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var m map[string]any
	if err := json.Unmarshal(b, &m); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	decl := m["declaration"].(map[string]any)
	scopes, ok := decl["scopes"].([]any)
	if !ok {
		t.Fatalf("scopes must be a JSON array, got %T in %s", decl["scopes"], b)
	}
	if len(scopes) != 0 {
		t.Fatalf("expected empty scopes array, got %v", scopes)
	}
}

func assertJSONEqual(t *testing.T, a, b []byte) {
	t.Helper()
	var am, bm any
	if err := json.Unmarshal(a, &am); err != nil {
		t.Fatalf("unmarshal got: %v\n%s", err, a)
	}
	if err := json.Unmarshal(b, &bm); err != nil {
		t.Fatalf("unmarshal golden: %v", err)
	}
	if !reflect.DeepEqual(am, bm) {
		ap, _ := json.MarshalIndent(am, "", "  ")
		bp, _ := json.MarshalIndent(bm, "", "  ")
		t.Fatalf("JSON mismatch.\n--- got ---\n%s\n--- want ---\n%s", ap, bp)
	}
}
