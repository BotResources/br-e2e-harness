BINARY := identity-passport
VECTORS := ../../crates/conformance-passport/vectors/passport-wire-v1.json

.PHONY: build vet test fmt check guard vectors clean

build:
	go build -o $(BINARY) .

vet:
	go vet ./...

test: vet
	go test ./...

fmt:
	gofmt -w .

guard:
	@if grep -rn --include='*.go' --exclude='*_test.go' 'json:"email"\|userIDFromEmail\|uuid.NewSHA1\|bearerTokenEntry\|BearerTokenEntry' . ; then echo 'RETIRED plaintext/email model present in the sealed anchor'; exit 1; fi

vectors:
	go run . vectors --out $(VECTORS)

check: fmt vet test guard

clean:
	rm -f $(BINARY)
