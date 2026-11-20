//go:build !windows

package main

func dpapiDecryptBytes(_ []byte) ([]byte, bool) {
	return nil, false
}
