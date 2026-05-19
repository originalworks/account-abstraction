import { network } from "hardhat";
const { ethers } = await network.connect();
import { KMSClient } from "@aws-sdk/client-kms";
import { fromIni } from "@aws-sdk/credential-providers";
import { KMS7702Signer } from "./kms7702/KMS7702Signer.js";

const SEOA_IMPLEMENTATION_ADDRESS =
  "0xF10A60D2394f6fc66b1E6967790F96E3F9b239d6";
const DDEX_SEQUENCER_ADDRESS = "0x75AbeCf07C26368F0f4AA0b0d3637A732E25467e";

async function main() {
  const kmsKeyId = process.env.KMS_KEY_ID!;
  const SEOA = await ethers.getContractFactory("sEOA");
  const kmsClient = new KMSClient({
    region: "us-east-1",
    // credentials: fromIni({ profile: "revelator-dev" }),
  });

  const kmsSigner = await KMS7702Signer.create(
    ethers.provider as any,
    kmsKeyId,
    kmsClient,
  );

  const auth = await kmsSigner.authorizeKms(
    {
      address: SEOA_IMPLEMENTATION_ADDRESS,
      nonce: (await kmsSigner.getNonce()) + 1,
    },
    kmsClient,
    kmsKeyId,
  );

  const tx = await kmsSigner.sendTransaction({
    to: kmsSigner.address,
    authorizationList: [auth],
    data: SEOA.interface.encodeFunctionData("setDdexSequencerAddress", [
      DDEX_SEQUENCER_ADDRESS,
    ]),
  });
  await tx.wait();
  console.log(tx.hash);
}

main();
