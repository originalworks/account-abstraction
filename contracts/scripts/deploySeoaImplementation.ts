import { KMSClient } from "@aws-sdk/client-kms";
import { fromIni } from "@aws-sdk/credential-providers";
import { KMSSigner } from "@rumblefishdev/eth-signer-kms";
import { network } from "hardhat";

const { ethers } = await network.connect();
const AWS_PROFILE = "";
const AWS_REGION = "";

async function main() {
  const kmsKeyId = process.env.KMS_KEY_ID!;

  const kmsClient = new KMSClient({
    region: AWS_REGION,
    credentials: fromIni({ profile: AWS_PROFILE }),
  });

  const kmsSigner = await KMSSigner.create(
    ethers.provider as any,
    kmsKeyId,
    kmsClient,
  );

  const SEOA = await ethers.getContractFactory("sEOA");

  const seoaImplementation = await SEOA.connect(kmsSigner as any).deploy();

  console.log(
    "Deployed Implementation Address: ",
    await seoaImplementation.getAddress(),
  );
}

main();
