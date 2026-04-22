import { network } from "hardhat";
import { SEOA__factory } from "../typechain/factories/SEOA__factory.js";
import { expect } from "chai";
import { HardhatEthersSigner } from "@nomicfoundation/hardhat-ethers/types";
import { SEOA } from "../typechain/SEOA.js";
import { HDNodeWallet } from "ethers";
import { ERC20TokenMock, ERC20TokenMock__factory } from "../typechain/index.js";
import { futureDeadline, randomSalt } from "./helpers.js";

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

describe("sEOA.sol (BLOB transactions)", () => {
  let deployer: HardhatEthersSigner;
  let gasSponsorA: HardhatEthersSigner;
  let gasSponsorB: HardhatEthersSigner;
  let delegatedAccount: HDNodeWallet;
  let sEOAimplementation: SEOA;
  let erc20Mock: ERC20TokenMock;
  let sEoa: SEOA;
  let ddexSequencerAddress: string;

  before(async () => {
    [deployer, gasSponsorA, gasSponsorB] = await ethers.getSigners();
    const FakeDdexSequencer = await ethers.getContractFactory(
      "FakeDdexSequencer",
    );
    const fakeDdexSequencer = await FakeDdexSequencer.deploy();
    ddexSequencerAddress = await fakeDdexSequencer.getAddress();
    const sEOA_factory = new SEOA__factory(deployer);
    sEOAimplementation = await sEOA_factory.deploy();
    await sEOAimplementation.waitForDeployment();
    const ERC20TokenMock = await ethers.getContractFactory("ERC20TokenMock");
    erc20Mock = await ERC20TokenMock.deploy();
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

  describe("execute() — success", function () {
    it.only("executes a valid signed payload and marks salt as used", async function () {
      const salt = randomSalt();
      const deadline = await futureDeadline(60);
      const imageId = randomSalt();
      const commitment = randomSalt();
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
      await sEoa.connect(gasSponsorA).sendBlobBatch([
        {
          imageId,
          commitment,
          blobSha2,
          salt,
          deadline,
          signature,
        },
      ]);
    });
  });
});
