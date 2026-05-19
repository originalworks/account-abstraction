import { Signature, hexlify, recoverAddress } from "ethers";

function readDerInteger(
  bytes: Uint8Array,
  offset: number,
): { value: Uint8Array; offset: number } {
  //
  // INTEGER tag
  //
  if (bytes[offset++] !== 0x02) {
    throw new Error("invalid DER integer");
  }

  const length = bytes[offset++];

  let value = bytes.slice(offset, offset + length);

  //
  // DER INTEGER is signed, so positive numbers
  // sometimes contain leading 0x00.
  //
  if (value[0] === 0x00) {
    value = value.slice(1);
  }

  return {
    value,
    offset: offset + length,
  };
}

export function parseKmsDerSignature(derSignature: Uint8Array): Signature {
  let offset = 0;

  //
  // SEQUENCE tag
  //
  if (derSignature[offset++] !== 0x30) {
    throw new Error("invalid DER sequence");
  }

  //
  // sequence length
  //
  offset++;

  const rResult = readDerInteger(derSignature, offset);

  const sResult = readDerInteger(derSignature, rResult.offset);

  const r = hexlify(rResult.value).slice(2).padStart(64, "0");

  const s = hexlify(sResult.value).slice(2).padStart(64, "0");

  return Signature.from({
    r: `0x${r}`,
    s: `0x${s}`,
    v: 27,
  });
}

export function withRecoveryBit(
  digest: string,
  sig: Signature,
  expectedAddress: string,
): Signature {
  for (const yParity of [0, 1]) {
    const candidate = Signature.from({
      r: sig.r,
      s: sig.s,
      yParity: yParity as 0 | 1,
    });

    const recovered = recoverAddress(digest, candidate);

    if (recovered.toLowerCase() === expectedAddress.toLowerCase()) {
      return candidate;
    }
  }

  throw new Error("cannot determine recovery bit");
}
