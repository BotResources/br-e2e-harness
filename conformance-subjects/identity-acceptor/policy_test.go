package main

import (
	"encoding/json"
	"reflect"
	"testing"
)

func declare(service string, scopeKeys ...string) declareServiceScopes {
	scopes := make([]rawScopeSpec, 0, len(scopeKeys))
	for _, key := range scopeKeys {
		scopes = append(scopes, rawScopeSpec{Key: key, LabelKey: "l", DescriptionKey: "d"})
	}
	return declareServiceScopes{Declaration: rawScopeDeclaration{
		Manifest: rawServiceManifest{Key: service, LabelKey: "l", DescriptionKey: "d"},
		Scopes:   scopes,
	}}
}

func TestCleanDeclarationIsAccepted(t *testing.T) {
	r := newRegistry()
	service, fault := r.judge(declare("notifier", "notifier:read", "notifier:admin"))
	if fault != nil {
		t.Fatalf("expected accept, got reject %s", fault.Reason)
	}
	if service != "notifier" {
		t.Fatalf("accepted service = %q, want notifier", service)
	}
}

func TestCrossServiceClaimRejectedAsPrefixMismatch(t *testing.T) {
	r := newRegistry()
	if _, fault := r.judge(declare("notifier", "notifier:read")); fault != nil {
		t.Fatalf("seed declaration must accept, got %s", fault.Reason)
	}
	_, fault := r.judge(declareServiceScopes{Declaration: rawScopeDeclaration{
		Manifest: rawServiceManifest{Key: "billing", LabelKey: "l", DescriptionKey: "d"},
		Scopes:   []rawScopeSpec{{Key: "notifier:read", LabelKey: "l", DescriptionKey: "d"}},
	}})
	if fault == nil {
		t.Fatalf("expected reject for cross-service claim")
	}
	if fault.Reason != "scope_prefix_mismatch" || fault.ScopeService != "notifier" || fault.DeclaringService != "billing" {
		t.Fatalf("unexpected fault %+v", fault)
	}
}

func TestRegistryCrossOwnerBranchMirrorsLib(t *testing.T) {
	r := newRegistry()
	r.ownerOf["notifier:read"] = "billing"
	fault := r.register(validatedDeclaration{
		service: "notifier",
		scopes:  []validatedScope{{key: "notifier:read", scopeService: "notifier"}},
	})
	if fault == nil || fault.Reason != "scope_owned_by_another_service" || fault.Owner != "billing" {
		t.Fatalf("unexpected fault %+v", fault)
	}
}

func TestIntraDeclarationDuplicateRejected(t *testing.T) {
	r := newRegistry()
	_, fault := r.judge(declare("notifier", "notifier:read", "notifier:read"))
	if fault == nil || fault.Reason != "duplicate_scope_in_declaration" || fault.Key != "notifier:read" {
		t.Fatalf("unexpected fault %+v", fault)
	}
}

func TestPrefixMismatchRejected(t *testing.T) {
	r := newRegistry()
	_, fault := r.judge(declare("notifier", "billing:read"))
	if fault == nil || fault.Reason != "scope_prefix_mismatch" {
		t.Fatalf("unexpected fault %+v", fault)
	}
	if fault.ScopeService != "billing" || fault.DeclaringService != "notifier" {
		t.Fatalf("unexpected fault %+v", fault)
	}
}

func TestInvalidScopeKeyRejected(t *testing.T) {
	r := newRegistry()
	_, fault := r.judge(declare("notifier", "notifier:BAD"))
	if fault == nil || fault.Reason != "invalid_scope_key" || fault.Validation.Validation != "invalid_charset" {
		t.Fatalf("unexpected fault %+v", fault)
	}
}

func TestMalformedScopeKeyRejected(t *testing.T) {
	r := newRegistry()
	_, fault := r.judge(declare("notifier", "notifierread"))
	if fault == nil || fault.Reason != "invalid_scope_key" || fault.Validation.Validation != "malformed_segments" {
		t.Fatalf("unexpected fault %+v", fault)
	}
	if fault.Key != "notifierread" {
		t.Fatalf("fault key = %q, want notifierread", fault.Key)
	}
}

func TestIdempotentRedeclarationAccepted(t *testing.T) {
	r := newRegistry()
	if _, fault := r.judge(declare("notifier", "notifier:read")); fault != nil {
		t.Fatalf("first declaration must accept, got %s", fault.Reason)
	}
	service, fault := r.judge(declare("notifier", "notifier:read"))
	if fault != nil {
		t.Fatalf("idempotent re-declare must accept, got %s", fault.Reason)
	}
	if service != "notifier" {
		t.Fatalf("re-declared service = %q, want notifier", service)
	}
}

func TestInvalidKeyTakesPrecedenceOverPrefixMismatch(t *testing.T) {
	r := newRegistry()
	_, fault := r.judge(declare("notifier", "billing:read", "bad key"))
	if fault == nil || fault.Reason != "invalid_scope_key" {
		t.Fatalf("invalid key must precede prefix mismatch, got %+v", fault)
	}
}

func TestRejectedReasonMatchesGoldenWire(t *testing.T) {
	cases := []struct {
		name   string
		fault  declarationFault
		golden string
	}{
		{
			"owned",
			declarationFault{Reason: "scope_owned_by_another_service", Key: "notifier:read", Owner: "billing"},
			`{"reason":"scope_owned_by_another_service","key":"notifier:read","owner":"billing"}`,
		},
		{
			"prefix",
			declarationFault{Reason: "scope_prefix_mismatch", ScopeService: "billing", DeclaringService: "notifier"},
			`{"reason":"scope_prefix_mismatch","scope_service":"billing","declaring_service":"notifier"}`,
		},
		{
			"duplicate",
			declarationFault{Reason: "duplicate_scope_in_declaration", Key: "notifier:read"},
			`{"reason":"duplicate_scope_in_declaration","key":"notifier:read"}`,
		},
		{
			"invalid_charset",
			invalidScopeKey("notifier:BAD", keyValidationFault{Validation: "invalid_charset"}),
			`{"reason":"invalid_scope_key","key":"notifier:BAD","validation":{"validation":"invalid_charset"}}`,
		},
		{
			"too_long",
			invalidScopeKey("x", keyValidationFault{Validation: "too_long", Max: 128, Actual: 200}),
			`{"reason":"invalid_scope_key","key":"x","validation":{"validation":"too_long","max":128,"actual":200}}`,
		},
		{
			"malformed_segments",
			invalidScopeKey("notifierread", keyValidationFault{Validation: "malformed_segments"}),
			`{"reason":"invalid_scope_key","key":"notifierread","validation":{"validation":"malformed_segments"}}`,
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got, err := json.Marshal(tc.fault)
			if err != nil {
				t.Fatalf("marshal: %v", err)
			}
			assertJSONEqual(t, got, []byte(tc.golden))
		})
	}
}

func TestRejectedEventWireShape(t *testing.T) {
	event := integrationEvent{
		EventID:    "0190a1b2-0000-7000-8000-000000000003",
		EventType:  rejectedType,
		Version:    wireVersion,
		OccurredAt: "2023-11-14T22:13:20Z",
		Metadata: messageMetadata{
			ActorID:       "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
			ActorKind:     "service",
			CorrelationID: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
		},
		Payload: serviceScopesRejected{
			Service: "notifier",
			Reason:  declarationFault{Reason: "scope_owned_by_another_service", Key: "notifier:read", Owner: "billing"},
		},
	}
	got, err := json.Marshal(event)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	golden := `{
      "event_id": "0190a1b2-0000-7000-8000-000000000003",
      "event_type": "service_scope.rejected",
      "version": 1,
      "occurred_at": "2023-11-14T22:13:20Z",
      "metadata": {
        "actor_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
        "actor_kind": "service",
        "correlation_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
      },
      "payload": {
        "service": "notifier",
        "reason": {
          "reason": "scope_owned_by_another_service",
          "key": "notifier:read",
          "owner": "billing"
        }
      }
    }`
	assertJSONEqual(t, got, []byte(golden))
}

func TestAcceptedEventWireShape(t *testing.T) {
	event := integrationEvent{
		EventID:    "0190a1b2-0000-7000-8000-000000000002",
		EventType:  acceptedType,
		Version:    wireVersion,
		OccurredAt: "2023-11-14T22:13:20Z",
		Metadata: messageMetadata{
			ActorID:       "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
			ActorKind:     "service",
			CorrelationID: "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
		},
		Payload: serviceScopesAccepted{Service: "notifier"},
	}
	got, err := json.Marshal(event)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	golden := `{
      "event_id": "0190a1b2-0000-7000-8000-000000000002",
      "event_type": "service_scope.accepted",
      "version": 1,
      "occurred_at": "2023-11-14T22:13:20Z",
      "metadata": {
        "actor_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
        "actor_kind": "service",
        "correlation_id": "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"
      },
      "payload": {"service": "notifier"}
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
