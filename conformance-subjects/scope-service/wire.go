package main

import "github.com/google/uuid"

// Wire shapes for the scope-declaration handshake v1.
// Frozen against docs/conformance/scope-wire-v1.md (golden JSON from the real
// br-rust-common types). Audit this file against that doc, nothing else.

const (
	subjectDeclare  = "identity.cmd.service_scope.declare.v1"
	subjectAccepted = "identity.evt.service_scope.accepted.v1"
	subjectRejected = "identity.evt.service_scope.rejected.v1"

	commandType = "service_scope.declare"
	wireVersion = 1
)

type messageMetadata struct {
	ActorID       string `json:"actor_id"`
	ActorKind     string `json:"actor_kind"`
	CorrelationID string `json:"correlation_id"`
	CausationID   string `json:"causation_id,omitempty"`
}

type rawServiceManifest struct {
	Key            string `json:"key"`
	LabelKey       string `json:"label_key"`
	DescriptionKey string `json:"description_key"`
}

type rawScopeSpec struct {
	Key            string `json:"key"`
	LabelKey       string `json:"label_key"`
	DescriptionKey string `json:"description_key"`
	PlatformOnly   bool   `json:"platform_only"`
}

type rawScopeDeclaration struct {
	Manifest rawServiceManifest `json:"manifest"`
	Scopes   []rawScopeSpec     `json:"scopes"`
}

type declareServiceScopes struct {
	Declaration rawScopeDeclaration `json:"declaration"`
}

type integrationCommand struct {
	CommandID   string               `json:"command_id"`
	CommandType string               `json:"command_type"`
	Version     uint8                `json:"version"`
	IssuedAt    string               `json:"issued_at"`
	Metadata    messageMetadata      `json:"metadata"`
	Payload     declareServiceScopes `json:"payload"`
}

// confirmationProbe mirrors the declarer-side CorrelationProbe: only metadata is
// needed to match. The confirmation's outcome is decided by the NATS subject it
// arrived on, not by its body, so we keep the payload as raw bytes for logging.
type confirmationProbe struct {
	Metadata messageMetadata `json:"metadata"`
}

// declaringActorNamespace and declaringActorID reproduce
// br-util-scope-declaration::declaring_actor — a deterministic v5 actor id per
// service key. The acceptor never validates it; we emit it for fidelity.
var declaringActorNamespace = uuid.MustParse("6f3a1c8e-4b27-4d59-9e10-a3f277c58d41")

func declaringActorID(serviceKey string) string {
	return uuid.NewSHA1(declaringActorNamespace, []byte(serviceKey)).String()
}
