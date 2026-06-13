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

type declarer struct {
	js        jetstream.JetStream
	cfg       config
	readiness *readiness
}

func newDeclarer(js jetstream.JetStream, cfg config, r *readiness) *declarer {
	return &declarer{js: js, cfg: cfg, readiness: r}
}

// run performs the boot-time scope-declaration handshake. It blocks until a
// terminal outcome (Accepted / Rejected) or ctx cancellation. Disabled mode
// sets readiness UP and returns immediately, publishing nothing.
func (d *declarer) run(ctx context.Context) error {
	if !d.cfg.enabled {
		log.Printf("scope-declaration disabled; readiness UP, publishing nothing")
		d.readiness.setReady()
		return nil
	}

	correlationID := uuid.NewString()
	command := d.buildCommand(correlationID)
	body, err := json.Marshal(command)
	if err != nil {
		return fmt.Errorf("marshal declare command: %w", err)
	}

	cons, err := d.createAwaiter(ctx)
	if err != nil {
		return fmt.Errorf("create awaiter (stream %q must exist — the subject never provisions it): %w", d.cfg.streamName, err)
	}

	matches := make(chan jetstream.Msg, 16)
	consCtx, err := cons.Consume(func(msg jetstream.Msg) {
		select {
		case matches <- msg:
		case <-ctx.Done():
		}
	})
	if err != nil {
		return fmt.Errorf("consume confirmations: %w", err)
	}
	defer consCtx.Stop()

	ticker := time.NewTicker(d.cfg.waitTimeout)
	defer ticker.Stop()

	d.publish(ctx, body, correlationID)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
			log.Printf("no confirmation for correlation_id=%s within %s; re-publishing same id", correlationID, d.cfg.waitTimeout)
			d.publish(ctx, body, correlationID)
		case msg := <-matches:
			if done := d.handleConfirmation(msg, correlationID); done {
				return nil
			}
		}
	}
}

func (d *declarer) buildCommand(correlationID string) integrationCommand {
	return integrationCommand{
		CommandID:   uuid.NewString(),
		CommandType: commandType,
		Version:     wireVersion,
		IssuedAt:    time.Now().UTC().Format(time.RFC3339Nano),
		Metadata: messageMetadata{
			ActorID:       declaringActorID(d.cfg.serviceKey),
			ActorKind:     "service",
			CorrelationID: correlationID,
		},
		Payload: d.cfg.declaration(),
	}
}

func (d *declarer) createAwaiter(ctx context.Context) (jetstream.Consumer, error) {
	return d.js.CreateConsumer(ctx, d.cfg.streamName, jetstream.ConsumerConfig{
		FilterSubjects:    []string{subjectAccepted, subjectRejected},
		DeliverPolicy:     jetstream.DeliverNewPolicy,
		AckPolicy:         jetstream.AckNonePolicy,
		InactiveThreshold: 5 * time.Minute,
	})
}

func (d *declarer) publish(ctx context.Context, body []byte, correlationID string) {
	if _, err := d.js.Publish(ctx, subjectDeclare, body); err != nil {
		log.Printf("declare publish failed (will retry after wait): correlation_id=%s err=%v", correlationID, err)
		return
	}
	log.Printf("published declare command correlation_id=%s subject=%s", correlationID, subjectDeclare)
}

// handleConfirmation returns true when a terminal outcome was reached.
func (d *declarer) handleConfirmation(msg jetstream.Msg, correlationID string) bool {
	var probe confirmationProbe
	if err := json.Unmarshal(msg.Data(), &probe); err != nil {
		return false
	}
	if probe.Metadata.CorrelationID != correlationID {
		return false
	}

	switch msg.Subject() {
	case subjectAccepted:
		log.Printf("scope declaration ACCEPTED correlation_id=%s; readiness UP", correlationID)
		d.readiness.setReady()
		return true
	case subjectRejected:
		reason := extractReasonCode(msg.Data())
		log.Printf("scope declaration REJECTED correlation_id=%s reason=%s; readiness DOWN, no retry", correlationID, reason)
		d.readiness.setNotReady(fmt.Sprintf("scope declaration rejected: %s", reason))
		return true
	default:
		return false
	}
}

func extractReasonCode(data []byte) string {
	var envelope struct {
		Payload struct {
			Reason struct {
				Reason string `json:"reason"`
			} `json:"reason"`
		} `json:"payload"`
	}
	if err := json.Unmarshal(data, &envelope); err != nil {
		return "undecodable"
	}
	if envelope.Payload.Reason.Reason == "" {
		return "unspecified"
	}
	return envelope.Payload.Reason.Reason
}
