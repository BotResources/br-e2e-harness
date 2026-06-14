package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"time"

	"github.com/google/uuid"
	"github.com/nats-io/nats.go/jetstream"
)

type acceptor struct {
	js        jetstream.JetStream
	cfg       config
	readiness *readiness
	registry  *registry
}

func newAcceptor(js jetstream.JetStream, cfg config, r *readiness) *acceptor {
	return &acceptor{js: js, cfg: cfg, readiness: r, registry: newRegistry()}
}

func (a *acceptor) run(ctx context.Context) error {
	if !a.cfg.enabled {
		log.Printf("scope-acceptance disabled; readiness UP, consuming nothing")
		a.readiness.setReady()
		<-ctx.Done()
		return ctx.Err()
	}

	cons, err := a.js.CreateConsumer(ctx, a.cfg.streamName, jetstream.ConsumerConfig{
		FilterSubjects:    []string{subjectDeclare},
		DeliverPolicy:     jetstream.DeliverNewPolicy,
		AckPolicy:         jetstream.AckNonePolicy,
		InactiveThreshold: 5 * time.Minute,
	})
	if err != nil {
		return fmt.Errorf("create declare consumer (stream %q must exist — the subject never provisions it): %w", a.cfg.streamName, err)
	}

	commands := make(chan jetstream.Msg, 64)
	consCtx, err := cons.Consume(func(msg jetstream.Msg) {
		select {
		case commands <- msg:
		case <-ctx.Done():
		}
	})
	if err != nil {
		return fmt.Errorf("consume declares: %w", err)
	}
	defer consCtx.Stop()

	a.readiness.setReady()
	log.Printf("acceptor consuming %s on stream %s; readiness UP", subjectDeclare, a.cfg.streamName)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case msg := <-commands:
			a.handleDeclare(ctx, msg)
		}
	}
}

func (a *acceptor) handleDeclare(ctx context.Context, msg jetstream.Msg) {
	var command integrationCommand
	if err := json.Unmarshal(msg.Data(), &command); err != nil {
		log.Printf("undecodable declare ignored: %v", err)
		return
	}
	correlationID := command.Metadata.CorrelationID
	if correlationID == "" {
		log.Printf("declare without correlation_id ignored")
		return
	}

	service, fault := a.registry.judge(command.Payload)
	if fault != nil {
		a.publishRejected(ctx, command.Payload.Declaration.Manifest.Key, *fault, correlationID)
		return
	}
	a.publishAccepted(ctx, service, correlationID)
}

func (a *acceptor) publishAccepted(ctx context.Context, service, correlationID string) {
	event := integrationEvent{
		EventID:    uuid.NewString(),
		EventType:  acceptedType,
		Version:    wireVersion,
		OccurredAt: time.Now().UTC().Format(time.RFC3339Nano),
		Metadata:   a.replyMetadata(correlationID),
		Payload:    serviceScopesAccepted{Service: service},
	}
	a.publish(ctx, subjectAccepted, event, correlationID)
	log.Printf("ACCEPTED service=%s correlation_id=%s", service, correlationID)
}

func (a *acceptor) publishRejected(ctx context.Context, service string, reason declarationFault, correlationID string) {
	event := integrationEvent{
		EventID:    uuid.NewString(),
		EventType:  rejectedType,
		Version:    wireVersion,
		OccurredAt: time.Now().UTC().Format(time.RFC3339Nano),
		Metadata:   a.replyMetadata(correlationID),
		Payload:    serviceScopesRejected{Service: service, Reason: reason},
	}
	a.publish(ctx, subjectRejected, event, correlationID)
	log.Printf("REJECTED service=%s reason=%s correlation_id=%s", service, reason.Reason, correlationID)
}

func (a *acceptor) replyMetadata(correlationID string) messageMetadata {
	return messageMetadata{
		ActorID:       uuid.NewString(),
		ActorKind:     "service",
		CorrelationID: correlationID,
	}
}

func (a *acceptor) publish(ctx context.Context, subject string, event integrationEvent, correlationID string) {
	body, err := json.Marshal(event)
	if err != nil {
		log.Printf("marshal confirmation failed correlation_id=%s: %v", correlationID, err)
		return
	}
	if _, err := a.js.Publish(ctx, subject, body); err != nil {
		log.Printf("publish confirmation failed subject=%s correlation_id=%s: %v", subject, correlationID, err)
	}
}
