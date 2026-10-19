#!/usr/bin/env bash
export TRACKED_LIBRARIES="oidc-test-idp br-test-harness conformance-directory conformance-identity conformance-nats-fabric conformance-passport conformance-scope"
export SEMVER_GATED_LIBRARIES="oidc-test-idp br-test-harness"
export COMPANION_CRATES="conformance-scope-cli:conformance-scope"
export COMPANION_CRATES="${COMPANION_CRATES} conformance-passport-cli:conformance-passport"
