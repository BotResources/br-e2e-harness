BINARY := identity-acceptor

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
	@if grep -rn --exclude=Makefile --exclude='*_test.go' 'identity\.cmd\.\|identity\.evt\.' . ; then echo 'DEAD GRAMMAR in the live-wire anchor'; exit 1; fi

check: fmt vet test guard

clean:
	rm -f $(BINARY)
