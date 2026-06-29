BINARY := identity-passport

.PHONY: build vet test fmt check guard clean

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

check: fmt vet test guard

clean:
	rm -f $(BINARY)
