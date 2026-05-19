import { KMSClient, SignCommand } from "@aws-sdk/client-kms";
import { KMSSigner } from "@rumblefishdev/eth-signer-kms";
import { Authorization } from "ethers";

import {
  AuthorizationRequest,
  concat,
  encodeRlp,
  getBytes,
  keccak256,
  resolveProperties,
  Signature,
  toBeHex,
} from "ethers";
import { parseKmsDerSignature, withRecoveryBit } from "./DER.js";

export class KMS7702Signer extends KMSSigner {
  kmsClient: KMSClient;
  kmsKeyId: string;

  constructor(kmsClient: KMSClient, kmsKeyId: string, ...args: any[]) {
    // @ts-ignore
    super(...args);

    this.kmsClient = kmsClient;
    this.kmsKeyId = kmsKeyId;
  }

  static async create(
    provider: any,
    keyId: string,
    kmsClient: KMSClient,
  ): Promise<KMS7702Signer> {
    const signer = await KMSSigner.create(provider, keyId, kmsClient);

    Object.setPrototypeOf(signer, KMS7702Signer.prototype);

    // signer.kmsClient = kmsClient;
    // signer.kmsKeyId = keyId;

    return signer as KMS7702Signer;
  }

  public async authorizeKms(
    authorization: AuthorizationRequest,
    kmsClient: KMSClient,
    kmsKeyId: string,
  ): Promise<Authorization> {
    const provider = this.provider;

    if (!provider) {
      throw new Error("missing provider");
    }

    const chainId = (await provider.getNetwork()).chainId;

    const resolved = await resolveProperties({
      address: authorization.address.toString(),
      nonce: BigInt(authorization.nonce!),
      chainId: chainId,
    });

    const encoded = encodeRlp([
      toBeHex(resolved.chainId),
      resolved.address,
      toBeHex(resolved.nonce),
    ]);

    const digest = keccak256(concat(["0x05", encoded]));

    const derSignature = await signDigest(kmsClient, kmsKeyId, digest);
    let sig = parseKmsDerSignature(derSignature);

    sig = Signature.from(sig);

    sig = withRecoveryBit(digest, sig, await this.getAddress());

    return {
      ...resolved,
      signature: sig,
    };
  }
}

async function signDigest(kmsClient: KMSClient, keyId: string, digest: string) {
  const res = await kmsClient.send(
    new SignCommand({
      KeyId: keyId,
      Message: getBytes(digest),
      MessageType: "DIGEST",
      SigningAlgorithm: "ECDSA_SHA_256",
    }),
  );

  if (!res.Signature) {
    throw new Error("missing signature");
  }

  return res.Signature;
}
