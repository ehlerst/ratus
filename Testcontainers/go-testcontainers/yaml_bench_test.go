package main

import (
	"fmt"
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

type Config struct {
	Metrics   bool             `yaml:"metrics"`
	Endpoints []EndpointConfig `yaml:"endpoints"`
}

type EndpointConfig struct {
	Name       string   `yaml:"name"`
	Group      string   `yaml:"group"`
	URL        string   `yaml:"url"`
	Interval   string   `yaml:"interval"`
	Conditions []string `yaml:"conditions"`
}

func generateYAML(count int) string {
	var b strings.Builder
	b.WriteString("metrics: true\nendpoints:\n")
	for i := 0; i < count; i++ {
		b.WriteString(fmt.Sprintf("  - name: service-%d\n    group: cluster-%d\n    url: \"https://api.internal/v1/service-%d/health\"\n    interval: 30s\n    conditions:\n      - \"[STATUS] == 200\"\n      - \"[RESPONSE_TIME] < 250\"\n", i, i%10, i))
	}
	return b.String()
}

func BenchmarkConfigIngestion10(b *testing.B) {
	data := []byte(generateYAML(10))
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var cfg Config
		_ = yaml.Unmarshal(data, &cfg)
	}
}

func BenchmarkConfigIngestion100(b *testing.B) {
	data := []byte(generateYAML(100))
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var cfg Config
		_ = yaml.Unmarshal(data, &cfg)
	}
}

func BenchmarkConfigIngestion1000(b *testing.B) {
	data := []byte(generateYAML(1000))
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var cfg Config
		_ = yaml.Unmarshal(data, &cfg)
	}
}
