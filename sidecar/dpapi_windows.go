//go:build windows

package main

import (
	"syscall"
	"unsafe"
)

var (
	modCrypt32             = syscall.NewLazyDLL("crypt32.dll")
	modKernel32            = syscall.NewLazyDLL("kernel32.dll")
	procCryptUnprotectData = modCrypt32.NewProc("CryptUnprotectData")
	procLocalFree          = modKernel32.NewProc("LocalFree")
)

type dataBlob struct {
	cbData uint32
	pbData *byte
}

func dpapiDecryptBytes(cipher []byte) ([]byte, bool) {
	if len(cipher) == 0 {
		return nil, false
	}
	entropy := []byte("KyoDesktopTokenSecurityV1")
	inBlob := dataBlob{cbData: uint32(len(cipher)), pbData: &cipher[0]}
	entropyBlob := dataBlob{cbData: uint32(len(entropy)), pbData: &entropy[0]}
	var outBlob dataBlob

	const cryptProtectUIForbidden = 0x1
	r1, _, _ := procCryptUnprotectData.Call(
		uintptr(unsafe.Pointer(&inBlob)),
		0,
		uintptr(unsafe.Pointer(&entropyBlob)),
		0,
		0,
		cryptProtectUIForbidden,
		uintptr(unsafe.Pointer(&outBlob)),
	)
	if r1 == 0 || outBlob.pbData == nil {
		return nil, false
	}
	defer procLocalFree.Call(uintptr(unsafe.Pointer(outBlob.pbData)))
	out := unsafe.Slice(outBlob.pbData, outBlob.cbData)
	res := make([]byte, len(out))
	copy(res, out)
	return res, true
}
