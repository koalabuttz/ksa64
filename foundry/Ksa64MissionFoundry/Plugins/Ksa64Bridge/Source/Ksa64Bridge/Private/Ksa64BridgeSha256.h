#pragma once

#include "CoreMinimal.h"

// Small portable SHA-256 implementation kept at the bridge boundary so a
// staged library is verified before any platform loader is asked to open it.
// It avoids Windows BCrypt and has no simulation role.
namespace Ksa64BridgeHash
{
namespace Private
{
constexpr uint32 RoundConstants[64] = {
    0x428A2F98u, 0x71374491u, 0xB5C0FBCFu, 0xE9B5DBA5u,
    0x3956C25Bu, 0x59F111F1u, 0x923F82A4u, 0xAB1C5ED5u,
    0xD807AA98u, 0x12835B01u, 0x243185BEu, 0x550C7DC3u,
    0x72BE5D74u, 0x80DEB1FEu, 0x9BDC06A7u, 0xC19BF174u,
    0xE49B69C1u, 0xEFBE4786u, 0x0FC19DC6u, 0x240CA1CCu,
    0x2DE92C6Fu, 0x4A7484AAu, 0x5CB0A9DCu, 0x76F988DAu,
    0x983E5152u, 0xA831C66Du, 0xB00327C8u, 0xBF597FC7u,
    0xC6E00BF3u, 0xD5A79147u, 0x06CA6351u, 0x14292967u,
    0x27B70A85u, 0x2E1B2138u, 0x4D2C6DFCu, 0x53380D13u,
    0x650A7354u, 0x766A0ABBu, 0x81C2C92Eu, 0x92722C85u,
    0xA2BFE8A1u, 0xA81A664Bu, 0xC24B8B70u, 0xC76C51A3u,
    0xD192E819u, 0xD6990624u, 0xF40E3585u, 0x106AA070u,
    0x19A4C116u, 0x1E376C08u, 0x2748774Cu, 0x34B0BCB5u,
    0x391C0CB3u, 0x4ED8AA4Au, 0x5B9CCA4Fu, 0x682E6FF3u,
    0x748F82EEu, 0x78A5636Fu, 0x84C87814u, 0x8CC70208u,
    0x90BEFFFAu, 0xA4506CEBu, 0xBEF9A3F7u, 0xC67178F2u,
};

FORCEINLINE uint32 RotateRight(uint32 Value, uint32 Shift)
{
    return (Value >> Shift) | (Value << (32u - Shift));
}

inline void Transform(uint32 State[8], const uint8 Block[64])
{
    uint32 Words[64] = {};
    for (uint32 Index = 0; Index < 16; ++Index)
    {
        const uint32 Offset = Index * 4;
        Words[Index] = (static_cast<uint32>(Block[Offset]) << 24)
            | (static_cast<uint32>(Block[Offset + 1]) << 16)
            | (static_cast<uint32>(Block[Offset + 2]) << 8)
            | static_cast<uint32>(Block[Offset + 3]);
    }
    for (uint32 Index = 16; Index < 64; ++Index)
    {
        const uint32 Sigma0 = RotateRight(Words[Index - 15], 7)
            ^ RotateRight(Words[Index - 15], 18)
            ^ (Words[Index - 15] >> 3);
        const uint32 Sigma1 = RotateRight(Words[Index - 2], 17)
            ^ RotateRight(Words[Index - 2], 19)
            ^ (Words[Index - 2] >> 10);
        Words[Index] = Words[Index - 16] + Sigma0 + Words[Index - 7] + Sigma1;
    }

    uint32 A = State[0]; uint32 B = State[1]; uint32 C = State[2]; uint32 D = State[3];
    uint32 E = State[4]; uint32 F = State[5]; uint32 G = State[6]; uint32 H = State[7];
    for (uint32 Index = 0; Index < 64; ++Index)
    {
        const uint32 UpperSigma1 = RotateRight(E, 6) ^ RotateRight(E, 11) ^ RotateRight(E, 25);
        const uint32 Choice = (E & F) ^ (~E & G);
        const uint32 Temporary1 = H + UpperSigma1 + Choice + RoundConstants[Index] + Words[Index];
        const uint32 UpperSigma0 = RotateRight(A, 2) ^ RotateRight(A, 13) ^ RotateRight(A, 22);
        const uint32 Majority = (A & B) ^ (A & C) ^ (B & C);
        const uint32 Temporary2 = UpperSigma0 + Majority;
        H = G; G = F; F = E; E = D + Temporary1; D = C; C = B; B = A; A = Temporary1 + Temporary2;
    }
    State[0] += A; State[1] += B; State[2] += C; State[3] += D;
    State[4] += E; State[5] += F; State[6] += G; State[7] += H;
}
}

inline FString Sha256Hex(const uint8* Data, uint64 Length)
{
    if (Data == nullptr && Length != 0) return {};
    uint32 State[8] = {
        0x6A09E667u, 0xBB67AE85u, 0x3C6EF372u, 0xA54FF53Au,
        0x510E527Fu, 0x9B05688Cu, 0x1F83D9ABu, 0x5BE0CD19u,
    };
    uint64 Offset = 0;
    while (Length - Offset >= 64) { Private::Transform(State, Data + Offset); Offset += 64; }

    uint8 Tail[128] = {};
    const uint32 Remaining = static_cast<uint32>(Length - Offset);
    if (Remaining != 0) FMemory::Memcpy(Tail, Data + Offset, Remaining);
    Tail[Remaining] = 0x80;
    const uint32 PaddingLength = Remaining < 56 ? 64 : 128;
    const uint64 BitLength = Length * 8;
    for (uint32 Index = 0; Index < 8; ++Index) Tail[PaddingLength - 1 - Index] = static_cast<uint8>(BitLength >> (Index * 8));
    Private::Transform(State, Tail);
    if (PaddingLength == 128) Private::Transform(State, Tail + 64);

    static constexpr TCHAR Digits[] = TEXT("0123456789abcdef");
    FString Output;
    Output.Reserve(64);
    for (const uint32 Word : State)
        for (int32 Shift = 28; Shift >= 0; Shift -= 4)
            Output.AppendChar(Digits[(Word >> Shift) & 0x0Fu]);
    return Output;
}
}