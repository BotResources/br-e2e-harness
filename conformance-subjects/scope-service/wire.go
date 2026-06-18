package main

import "github.com/google/uuid"

const (
	subjectDeclare  = "integration.cmd.identity.service_scope.declare.v1"
	subjectAccepted = "integration.evt.identity.service_scope.accepted.v1"
	subjectRejected = "integration.evt.identity.service_scope.rejected.v1"

	eventStream = "INTEGRATION_EVT"

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

type confirmationProbe struct {
	Metadata messageMetadata `json:"metadata"`
}

var declaringActorNamespace = uuid.MustParse("6f3a1c8e-4b27-4d59-9e10-a3f277c58d41")

func declaringActorID(serviceKey string) string {
	return uuid.NewSHA1(declaringActorNamespace, []byte(serviceKey)).String()
}
