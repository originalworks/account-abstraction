import { network } from "hardhat";
const { ethers } = await network.connect();
import { KMSClient } from "@aws-sdk/client-kms";
import { fromIni } from "@aws-sdk/credential-providers";
import { KMSSigner } from "@rumblefishdev/eth-signer-kms";

const SEOA_IMPLEMENTATION_ADDRESS = "";
const DDEX_SEQUENCER_ADDRESS = "";
const AWS_PROFILE = "";
const AWS_REGION = "";

async function main() {
  const kmsKeyId = process.env.KMS_KEY_ID!;

  const SEOA = await ethers.getContractFactory("sEOA");
  const kmsClient = new KMSClient({
    region: AWS_REGION,
    credentials: fromIni({ profile: AWS_PROFILE }),
  });

  const kmsSigner = await KMSSigner.create(
    ethers.provider as any,
    kmsKeyId,
    kmsClient,
  );

  console.log(kmsSigner.address);

  const auth = await kmsSigner.authorize({
    address: SEOA_IMPLEMENTATION_ADDRESS,
    nonce: (await kmsSigner.getNonce()) + 1,
  });

  const tx = await kmsSigner.sendTransaction({
    to: kmsSigner.address,
    authorizationList: [auth],
    data: SEOA.interface.encodeFunctionData("setDdexSequencerAddress", [
      DDEX_SEQUENCER_ADDRESS,
    ]),
  });
  await tx.wait();
  console.log("Transaction sent as:", tx.hash);
}

main();
