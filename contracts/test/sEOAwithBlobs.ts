import { network } from "hardhat";
import { SEOA__factory } from "../typechain/factories/SEOA__factory.js";
import { expect } from "chai";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/types";
import { SEOA } from "../typechain/SEOA.js";
import { HDNodeWallet, parseEther, Wallet } from "ethers";
import { ERC20TokenMock, ERC20TokenMock__factory } from "../typechain/index.js";
import {
  futureDeadline,
  getCurrentTimestamp,
  getEthersType3Wallets,
  randomSalt,
} from "./helpers.js";
import { KzgHelper } from "./kzg.js";

const { ethers } = await network.connect();

interface BuildSignatureInput {
  signer: HardhatEthersSigner;
  sEoa: SEOA;
  imageId: string;
  commitment: string;
  blobSha2: string;
  salt: string;
  deadline: number;
  chainId?: number;
}

// WIP!!!!

async function buildSignature(input: BuildSignatureInput) {
  const chainId = input.chainId || (await ethers.provider.getNetwork()).chainId;

  const domain = {
    name: "sEOA",
    version: "1",
    chainId,
    verifyingContract: await input.sEoa.getAddress(),
  };

  const types = {
    SignedBlobCall: [
      { name: "imageId", type: "bytes32" },
      { name: "commitmentHash", type: "bytes32" },
      { name: "blobSha2", type: "bytes32" },
      { name: "salt", type: "bytes32" },
      { name: "deadline", type: "uint256" },
    ],
  };

  const commitmentHash = ethers.keccak256(input.commitment);

  const value = {
    imageId: input.imageId,
    commitmentHash,
    blobSha2: input.blobSha2,
    salt: input.salt,
    deadline: input.deadline,
  };

  return input.signer.signTypedData(domain, types, value);
}

describe.only("sEOA.sol (BLOB transactions)", () => {
  let deployer: HardhatEthersSigner;
  let gasSponsorA: HDNodeWallet;
  let gasSponsorB: HDNodeWallet;
  let delegatedAccount: HDNodeWallet;
  let sEOAimplementation: SEOA;
  let sEoa: SEOA;
  let ddexSequencerAddress: string;
  let kzgHelper: KzgHelper;

  before(async () => {
    let fundsSource;
    [deployer, fundsSource] = await ethers.getSigners();
    const type3Wallets = await getEthersType3Wallets({
      fundsSource,
      numberOfWallets: 2,
      prefundValue: parseEther("1"),
    });

    gasSponsorA = type3Wallets[0].connect(ethers.provider);
    gasSponsorB = type3Wallets[1].connect(ethers.provider);

    const FakeDdexSequencer = await ethers.getContractFactory(
      "FakeDdexSequencer",
    );
    const fakeDdexSequencer = await FakeDdexSequencer.deploy();
    ddexSequencerAddress = await fakeDdexSequencer.getAddress();
    const sEOA_factory = new SEOA__factory(deployer);
    sEOAimplementation = await sEOA_factory.deploy();
    await sEOAimplementation.waitForDeployment();
    kzgHelper = new KzgHelper();
    await kzgHelper.generate();
  });
  beforeEach(async () => {
    delegatedAccount = ethers.Wallet.createRandom().connect(ethers.provider);
    await deployer.sendTransaction({
      to: delegatedAccount,
      value: ethers.parseEther("1"),
    });

    const auth = await delegatedAccount.authorize({
      address: await sEOAimplementation.getAddress(),
      nonce: (await delegatedAccount.getNonce()) + 1,
    });

    const tx = await delegatedAccount.sendTransaction({
      to: delegatedAccount.address,
      authorizationList: [auth],
      data: SEOA__factory.createInterface().encodeFunctionData(
        "setDdexSequencerAddress",
        [ddexSequencerAddress],
      ),
    });
    await tx.wait();

    sEoa = SEOA__factory.connect(delegatedAccount.address, delegatedAccount);
  });

  describe("sendBlobBatch() — success", function () {
    it("executes a valid signed payload and marks salt as used", async function () {
      const calculatedBlob = kzgHelper.calculatedBlobs[0];
      const salt = randomSalt();
      const deadline = await futureDeadline(60);
      const imageId = randomSalt();
      const commitment = calculatedBlob.commitment;
      const blobSha2 = randomSalt();

      const signature = await buildSignature({
        signer: delegatedAccount,
        sEoa,
        imageId,
        commitment,
        blobSha2,
        salt,
        deadline,
      });

      await expect(
        sEoa.connect(gasSponsorA).sendBlobBatch(
          [
            {
              imageId,
              commitment,
              blobSha2,
              salt,
              deadline,
              signature,
            },
          ],
          {
            type: 3,
            maxFeePerBlobGas: 10,
            gasLimit: 1000000,
            blobs: [
              {
                data: calculatedBlob.blobString,
                proof: calculatedBlob.proof,
                commitment: commitment,
              },
            ],
          },
        ),
      ).to.not.revert(ethers);
      expect(await sEoa.usedSalts(salt)).to.eq(true);
    });

    it("Can send two in a batch", async function () {
      const imageId = randomSalt();
      const calculatedBlobA = kzgHelper.calculatedBlobs[0];
      const calculatedBlobB = kzgHelper.calculatedBlobs[0];
      const saltA = randomSalt();
      const saltB = randomSalt();
      const deadline = await futureDeadline(3600);
      const commitmentA = calculatedBlobA.commitment;
      const commitmentB = calculatedBlobB.commitment;
      const blobSha2A = randomSalt();
      const blobSha2B = randomSalt();

      const signatureA = await buildSignature({
        signer: delegatedAccount,
        sEoa,
        imageId,
        commitment: commitmentA,
        blobSha2: blobSha2A,
        salt: saltA,
        deadline,
      });

      const signatureB = await buildSignature({
        signer: delegatedAccount,
        sEoa,
        imageId,
        commitment: commitmentB,
        blobSha2: blobSha2B,
        salt: saltB,
        deadline,
      });

      await expect(
        sEoa.connect(gasSponsorA).sendBlobBatch(
          [
            {
              imageId,
              commitment: commitmentA,
              blobSha2: blobSha2A,
              salt: saltA,
              deadline,
              signature: signatureA,
            },
            {
              imageId,
              commitment: commitmentB,
              blobSha2: blobSha2B,
              salt: saltB,
              deadline,
              signature: signatureB,
            },
          ],
          {
            type: 3,
            maxFeePerBlobGas: 10,
            gasLimit: 1000000,
            blobs: [
              {
                data: calculatedBlobA.blobString,
                proof: calculatedBlobA.proof,
                commitment: commitmentA,
              },
              {
                data: calculatedBlobB.blobString,
                proof: calculatedBlobB.proof,
                commitment: commitmentB,
              },
            ],
          },
        ),
      ).to.not.revert(ethers);
      expect(await sEoa.usedSalts(saltA)).to.eq(true);
      expect(await sEoa.usedSalts(saltB)).to.eq(true);
    });
  });
  it("reverts with AlreadyUsed on salt replay", async () => {
    const calculatedBlob = kzgHelper.calculatedBlobs[0];
    const salt = randomSalt();
    const deadline = await futureDeadline(60);
    const imageId = randomSalt();
    const commitment = calculatedBlob.commitment;
    const blobSha2 = randomSalt();

    const signature = await buildSignature({
      signer: delegatedAccount,
      sEoa,
      imageId,
      commitment,
      blobSha2,
      salt,
      deadline,
    });

    await expect(
      sEoa.connect(gasSponsorA).sendBlobBatch(
        [
          {
            imageId,
            commitment,
            blobSha2,
            salt,
            deadline,
            signature,
          },
        ],
        {
          type: 3,
          maxFeePerBlobGas: 10,
          gasLimit: 1000000,
          blobs: [
            {
              data: calculatedBlob.blobString,
              proof: calculatedBlob.proof,
              commitment: commitment,
            },
          ],
        },
      ),
    ).to.not.revert(ethers);
    expect(await sEoa.usedSalts(salt)).to.eq(true);

    await expect(
      sEoa.connect(gasSponsorA).sendBlobBatch(
        [
          {
            imageId,
            commitment,
            blobSha2,
            salt,
            deadline,
            signature,
          },
        ],
        {
          type: 3,
          maxFeePerBlobGas: 10,
          gasLimit: 1000000,
          blobs: [
            {
              data: calculatedBlob.blobString,
              proof: calculatedBlob.proof,
              commitment: commitment,
            },
          ],
        },
      ),
    ).to.be.revertedWithCustomError(sEoa, "AlreadyUsed");
  });

  it("reverts with Expired when deadline is in the past", async () => {
    const calculatedBlob = kzgHelper.calculatedBlobs[0];
    const salt = randomSalt();
    const now = await getCurrentTimestamp();
    const deadline = now - 1;
    const imageId = randomSalt();
    const commitment = calculatedBlob.commitment;
    const blobSha2 = randomSalt();

    const signature = await buildSignature({
      signer: delegatedAccount,
      sEoa,
      imageId,
      commitment,
      blobSha2,
      salt,
      deadline,
    });

    await expect(
      sEoa.connect(gasSponsorA).sendBlobBatch(
        [
          {
            imageId,
            commitment,
            blobSha2,
            salt,
            deadline,
            signature,
          },
        ],
        {
          type: 3,
          maxFeePerBlobGas: 10,
          gasLimit: 1000000,
          blobs: [
            {
              data: calculatedBlob.blobString,
              proof: calculatedBlob.proof,
              commitment: commitment,
            },
          ],
        },
      ),
    ).to.be.revertedWithCustomError(sEoa, "Expired");
  });
});
