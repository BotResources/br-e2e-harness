package main

type vectorSpec struct {
	name      string
	token     string
	actorKind string
	actorID   string
	tokenID   string
	wrongKey  bool
	twinOf    string
	mutation  string
	resolves  string
}

var (
	frozenSealKey = []byte{
		0x1f, 0x2e, 0x3d, 0x4c, 0x5b, 0x6a, 0x79, 0x88, 0x97, 0xa6, 0xb5, 0xc4, 0xd3, 0xe2, 0xf1, 0x00,
		0x0f, 0x1e, 0x2d, 0x3c, 0x4b, 0x5a, 0x69, 0x78, 0x87, 0x96, 0xa5, 0xb4, 0xc3, 0xd2, 0xe1, 0xf0,
	}
	frozenWrongSealKey = []byte{
		0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
		0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99,
	}
)

func frozenVectorSpecs() []vectorSpec {
	return []vectorSpec{
		{
			name:      "faithful-human",
			token:     "brk_conformance_faithful_human",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0001-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0001-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "faithful-human-second",
			token:     "brk_conformance_faithful_human_second",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0002-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0002-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "faithful-service",
			token:     "brk_conformance_faithful_service",
			actorKind: actorService,
			actorID:   "0190a1b2-0003-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0003-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesUnasserted,
		},
		{
			name:      "revoked",
			token:     "brk_conformance_revoked",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0004-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0004-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "kv-error",
			token:     "brk_conformance_kv_error",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0005-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0005-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "wrong-key",
			token:     "brk_conformance_wrong_key",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0006-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0006-7e5f-8a9b-0c1d2e3f4a5b",
			wrongKey:  true,
			resolves:  resolvesAnonymous,
		},
		{
			name:      "tampered-ciphertext-faithful",
			token:     "brk_conformance_tampered_ciphertext",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0007-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0007-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "tampered-ciphertext-corrupt",
			token:     "brk_conformance_tampered_ciphertext",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0007-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0007-7e5f-8a9b-0c1d2e3f4a5b",
			twinOf:    "tampered-ciphertext-faithful",
			mutation:  tamperCiphertext,
			resolves:  resolvesAnonymous,
		},
		{
			name:      "tampered-nonce-faithful",
			token:     "brk_conformance_tampered_nonce",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0008-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0008-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "tampered-nonce-corrupt",
			token:     "brk_conformance_tampered_nonce",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0008-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0008-7e5f-8a9b-0c1d2e3f4a5b",
			twinOf:    "tampered-nonce-faithful",
			mutation:  tamperNonce,
			resolves:  resolvesAnonymous,
		},
		{
			name:      "unreadable-faithful",
			token:     "brk_conformance_unreadable",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0009-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0009-7e5f-8a9b-0c1d2e3f4a5b",
			resolves:  resolvesHuman,
		},
		{
			name:      "unreadable-corrupt",
			token:     "brk_conformance_unreadable",
			actorKind: actorHuman,
			actorID:   "0190a1b2-0009-7e5f-8a9b-0c1d2e3f4a5b",
			tokenID:   "0190c0de-0009-7e5f-8a9b-0c1d2e3f4a5b",
			twinOf:    "unreadable-faithful",
			mutation:  corruptUnreadable,
			resolves:  resolvesAnonymous,
		},
	}
}
