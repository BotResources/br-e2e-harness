BINARY := identity-passport
VECTORS := ../../crates/conformance-passport/vectors/passport-wire-v1.json

.PHONY: build vet test fmt fmt-check check guard vectors clean

build:
	go build -o $(BINARY) .

vet:
	go vet ./...

test: vet
	go test -count=1 ./...

fmt:
	gofmt -w .

fmt-check:
	@unformatted="$$(gofmt -l .)"; \
	if [ -n "$$unformatted" ]; then echo "gofmt would rewrite:"; echo "$$unformatted"; exit 1; fi

guard:
	@if grep -rn --include='*.go' --exclude='*_test.go' 'json:"email"\|userIDFromEmail\|uuid.NewSHA1\|bearerTokenEntry\|BearerTokenEntry' . ; then echo 'RETIRED plaintext/email model present in the sealed anchor'; exit 1; fi

vectors:
	go run . vectors --out $(VECTORS)

check: fmt-check vet test guard

clean:
	rm -f $(BINARY)
