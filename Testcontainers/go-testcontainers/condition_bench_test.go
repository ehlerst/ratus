package main

import (
	"encoding/json"
	"strconv"
	"strings"
	"testing"
)

// Benchmark Go's condition evaluation pattern (Gatus replaces placeholders and evaluates)
func BenchmarkGoScalarStatusEval(b *testing.B) {
	statusCode := 200
	condition := "[STATUS] == 200"

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		actual := strconv.Itoa(statusCode)
		expr := strings.Replace(condition, "[STATUS]", actual, 1)
		parts := strings.Split(expr, "==")
		left := strings.TrimSpace(parts[0])
		right := strings.TrimSpace(parts[1])
		_ = left == right
	}
}

func BenchmarkGoJSONBodyEval(b *testing.B) {
	jsonBytes := []byte(`{"status": "UP", "data": {"code": 42, "items": [1, 2, 3]}}`)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var data map[string]interface{}
		_ = json.Unmarshal(jsonBytes, &data)
		d, ok := data["data"].(map[string]interface{})
		if ok {
			code, ok2 := d["code"].(float64)
			_ = ok2 && code == 42
		}
	}
}
