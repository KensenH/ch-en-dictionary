package main

import "testing"

func TestParseCedictLine(t *testing.T) {
	traditional, simplified, pinyin, definition, err := parseCedictLine(
		"你好 你好 [ni3 hao3] /hello/hi/",
	)
	if err != nil {
		t.Fatalf("parseCedictLine returned an error: %v", err)
	}

	if traditional != "你好" || simplified != "你好" || pinyin != "ni3 hao3" || definition != "hello/hi" {
		t.Fatalf(
			"unexpected parsed line: traditional=%q simplified=%q pinyin=%q definition=%q",
			traditional,
			simplified,
			pinyin,
			definition,
		)
	}
}

func TestParseCedictLineRejectsMalformedInput(t *testing.T) {
	_, _, _, _, err := parseCedictLine("malformed line")
	if err == nil {
		t.Fatal("parseCedictLine accepted malformed input")
	}
}

func TestValuePlaceholders(t *testing.T) {
	if got, want := valuePlaceholders(2, 3), "(?, ?, ?),(?, ?, ?)"; got != want {
		t.Fatalf("valuePlaceholders() = %q, want %q", got, want)
	}
}
