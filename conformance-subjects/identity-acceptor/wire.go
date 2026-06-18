package main

const (
	subjectDeclare  = "integration.cmd.identity.service_scope.declare.v1"
	subjectAccepted = "integration.evt.identity.service_scope.accepted.v1"
	subjectRejected = "integration.evt.identity.service_scope.rejected.v1"

	acceptedType = "service_scope.accepted"
	rejectedType = "service_scope.rejected"
	wireVersion  = 1
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

type serviceScopesAccepted struct {
	Service string `json:"service"`
}

type serviceScopesRejected struct {
	Service string           `json:"service"`
	Reason  declarationFault `json:"reason"`
}

type integrationEvent struct {
	EventID    string          `json:"event_id"`
	EventType  string          `json:"event_type"`
	Version    uint8           `json:"version"`
	OccurredAt string          `json:"occurred_at"`
	Metadata   messageMetadata `json:"metadata"`
	Payload    any             `json:"payload"`
}
