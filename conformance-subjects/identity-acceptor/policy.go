package main

import (
	"encoding/json"
	"strings"
)

const (
	serviceKeyMaxLen = 64
	scopeKeyMaxLen   = 128
)

type declarationFault struct {
	Reason           string
	Key              string
	Validation       *keyValidationFault
	ScopeService     string
	DeclaringService string
	Owner            string
}

type keyValidationFault struct {
	Validation string
	Max        int
	Actual     int
}

func (f declarationFault) MarshalJSON() ([]byte, error) {
	out := map[string]any{"reason": f.Reason}
	switch f.Reason {
	case "invalid_scope_key":
		out["key"] = f.Key
		out["validation"] = f.Validation
	case "scope_prefix_mismatch":
		out["scope_service"] = f.ScopeService
		out["declaring_service"] = f.DeclaringService
	case "duplicate_scope_in_declaration":
		out["key"] = f.Key
	case "scope_owned_by_another_service":
		out["key"] = f.Key
		out["owner"] = f.Owner
	}
	return json.Marshal(out)
}

func (v keyValidationFault) MarshalJSON() ([]byte, error) {
	out := map[string]any{"validation": v.Validation}
	if v.Validation == "too_long" {
		out["max"] = v.Max
		out["actual"] = v.Actual
	}
	return json.Marshal(out)
}

func invalidScopeKey(key string, validation keyValidationFault) declarationFault {
	return declarationFault{Reason: "invalid_scope_key", Key: key, Validation: &validation}
}

func validateSegment(value string, maxLen int) (keyValidationFault, bool) {
	if value == "" {
		return keyValidationFault{Validation: "empty"}, false
	}
	if len(value) > maxLen {
		return keyValidationFault{Validation: "too_long", Max: maxLen, Actual: len(value)}, false
	}
	for i := 0; i < len(value); i++ {
		b := value[i]
		ok := (b >= 'a' && b <= 'z') || (b >= '0' && b <= '9') || b == '_'
		if !ok {
			return keyValidationFault{Validation: "invalid_charset"}, false
		}
	}
	return keyValidationFault{}, true
}

func validateServiceKey(value string) (keyValidationFault, bool) {
	return validateSegment(value, serviceKeyMaxLen)
}

func validateScopeKey(value string) (string, keyValidationFault, bool) {
	if len(value) > scopeKeyMaxLen {
		return "", keyValidationFault{Validation: "too_long", Max: scopeKeyMaxLen, Actual: len(value)}, false
	}
	parts := strings.Split(value, ":")
	if len(parts) != 2 {
		return "", keyValidationFault{Validation: "malformed_segments"}, false
	}
	service, capability := parts[0], parts[1]
	if fault, ok := validateSegment(service, serviceKeyMaxLen); !ok {
		return "", fault, false
	}
	if fault, ok := validateSegment(capability, scopeKeyMaxLen); !ok {
		return "", fault, false
	}
	return service, keyValidationFault{}, true
}

type validatedDeclaration struct {
	service string
	scopes  []validatedScope
}

type validatedScope struct {
	key          string
	scopeService string
}

func validateDeclaration(decl rawScopeDeclaration) (validatedDeclaration, *declarationFault) {
	if fault, ok := validateServiceKey(decl.Manifest.Key); !ok {
		f := invalidScopeKey(decl.Manifest.Key, fault)
		return validatedDeclaration{}, &f
	}

	scopeServices := make([]string, 0, len(decl.Scopes))
	for _, spec := range decl.Scopes {
		scopeService, fault, ok := validateScopeKey(spec.Key)
		if !ok {
			f := invalidScopeKey(spec.Key, fault)
			return validatedDeclaration{}, &f
		}
		scopeServices = append(scopeServices, scopeService)
	}

	seen := make(map[string]struct{}, len(decl.Scopes))
	scopes := make([]validatedScope, 0, len(decl.Scopes))
	for i, spec := range decl.Scopes {
		scopeService := scopeServices[i]
		if scopeService != decl.Manifest.Key {
			return validatedDeclaration{}, &declarationFault{
				Reason:           "scope_prefix_mismatch",
				ScopeService:     scopeService,
				DeclaringService: decl.Manifest.Key,
			}
		}
		if _, dup := seen[spec.Key]; dup {
			return validatedDeclaration{}, &declarationFault{
				Reason: "duplicate_scope_in_declaration",
				Key:    spec.Key,
			}
		}
		seen[spec.Key] = struct{}{}
		scopes = append(scopes, validatedScope{key: spec.Key, scopeService: scopeService})
	}

	return validatedDeclaration{service: decl.Manifest.Key, scopes: scopes}, nil
}

type registry struct {
	ownerOf map[string]string
}

func newRegistry() *registry {
	return &registry{ownerOf: make(map[string]string)}
}

func (r *registry) register(decl validatedDeclaration) *declarationFault {
	for _, scope := range decl.scopes {
		if owner, claimed := r.ownerOf[scope.key]; claimed && owner != decl.service {
			return &declarationFault{
				Reason: "scope_owned_by_another_service",
				Key:    scope.key,
				Owner:  owner,
			}
		}
	}
	for _, scope := range decl.scopes {
		r.ownerOf[scope.key] = decl.service
	}
	return nil
}

func (r *registry) judge(payload declareServiceScopes) (string, *declarationFault) {
	declaration, fault := validateDeclaration(payload.Declaration)
	if fault != nil {
		return "", fault
	}
	if fault := r.register(declaration); fault != nil {
		return "", fault
	}
	return declaration.service, nil
}
