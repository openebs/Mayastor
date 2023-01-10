// Unit tests for nexus grpc api. Nexus is basically a hub which does IO
// replication to connected replicas. We test nexus operations with all
// supported replica types: nvmf, bdev, aio and uring. aio is not used
// in the product but it was part of initial implementation, so we keep it in
// case it would be useful in the future. uring was added later and is also
// not used in the product but kept for testing.

'use strict';

const assert = require('chai').assert;
const async = require('async');
const fs = require('fs');
const path = require('path');
const { exec } = require('child_process');
const grpc = require('grpc');
const common = require('./test_common');
const enums = require('./grpc_enums');
const url = require('url');
const NEXUSNAME = 'nexus0';
const NEXUSUUID = 'dbe4d7eb-118a-4d15-b789-a18d9af6ff20';
// just some UUID used for nexus ID
const UUID = 'dbe4d7eb-118a-4d15-b789-a18d9af6ff21';
const UUID2 = 'dbe4d7eb-118a-4d15-b789-a18d9af6ff22';
const TGTUUID = 'dbe4d7eb-118a-4d15-b789-a18d9af6ff29';
const BASEDEV = 'Malloc0';

// backend file for aio bdev
const aioFile = '/tmp/aio-backend';
// backend file for io_uring bdev
const uringFile = '/tmp/uring-backend';
// 64MB is the size of nexus and replicas
const diskSize = 64 * 1024 * 1024;
// external IP address detected by common lib
const externIp = common.getMyIp();

// The config just for nvmf target which cannot run in the same process as
// the nvmf initiator (SPDK limitation).
const configNvmfTarget = `
nexus_opts:
  nvmf_nexus_port: 4422
  nvmf_replica_port: 8420
nvmf_tcp_tgt_conf:
  max_namespaces: 2
`;

let client;

function controlPlaneTest (thisProtocol) {
  it('should publish the nexus', (done) => {
    client.publishNexus(
      {
        uuid: UUID,
        share: thisProtocol
      },
      (err, res) => {
        if (err) done(err);
        assert(res.device_uri);
        done();
      }
    );
  });

  it('should un-publish the nexus device', (done) => {
    client.unpublishNexus({ uuid: UUID }, (err, res) => {
      if (err) done(err);
      done();
    });
  });

  it('should re-publish the nexus', (done) => {
    client.publishNexus(
      {
        uuid: UUID,
        share: thisProtocol
      },
      (err, res) => {
        if (err) done(err);
        assert(res.device_uri);
        done();
      }
    );
  });

  it('should fail another publish request using a different protocol', (done) => {
    const differentProtocol = (thisProtocol === enums.NEXUS_NBD ? enums.NEXUS_NVMF : enums.NEXUS_NBD);
    client.publishNexus(
      {
        uuid: UUID,
        share: differentProtocol
      },
      (err, res) => {
        if (!err) return done(new Error('Expected error'));
        assert.equal(err.code, grpc.status.INVALID_ARGUMENT);
        done();
      }
    );
  });

  it('should succeed another publish request using the existing protocol', (done) => {
    client.publishNexus(
      {
        uuid: UUID,
        share: thisProtocol
      },
      (err, res) => {
        if (err) done(err);
        assert(res.device_uri);
        done();
      }
    );
  });

  it('should un-publish the nexus device', (done) => {
    client.unpublishNexus({ uuid: UUID }, (err, res) => {
      if (err) done(err);
      done();
    });
  });

  it('should succeed another un-publish request', (done) => {
    client.unpublishNexus({ uuid: UUID }, (err, res) => {
      if (err) done(err);
      done();
    });
  });

  it.skip('should re-publish the nexus using a crypto-key', (done) => {
    client.publishNexus(
      {
        uuid: UUID,
        share: thisProtocol,
        key: '0123456789123456'
      },
      (err, res) => {
        if (err) done(err);
        assert(res.device_uri);
        if (thisProtocol === enums.NEXUS_NVMF) {
          assert.equal(res.device_uri, `nvmf://${externIp}:8420/nqn.2019-05.io.openebs:crypto-nexus-${UUID}`);
        }
        done();
      }
    );
  });

  it('should un-publish the nexus device', (done) => {
    client.unpublishNexus({ uuid: UUID }, (err, res) => {
      if (err) done(err);
      done();
    });
  });
}

const doUring = (function () {
  let executed = false;
  let supportsUring = false;
  return function () {
    if (!executed) {
      executed = true;
      const { exec } = require('child_process');
      const URING_SUPPORT_CMD = path.join(
        __dirname,
        '..',
        '..',
        'target',
        'debug',
        'uring-support'
      );
      exec(URING_SUPPORT_CMD, (error) => {
        if (error) {
          return;
        }
        supportsUring = true;
      });
    }
    return supportsUring;
  };
})();

describe('nexus', function () {
  // TODO: use promisifyAll from grpc-promise to avoid these definitions
  const unpublish = (args) => {
    return new Promise((resolve, reject) => {
      client.unpublishNexus(args, (err, data) => {
        if (err) return reject(err);
        resolve(data);
      });
    });
  };

  const publish = (args) => {
    return new Promise((resolve, reject) => {
      client.publishNexus(args, (err, data) => {
        if (err) return reject(err);
        resolve(data);
      });
    });
  };

  const destroyNexus = (args) => {
    return new Promise((resolve, reject) => {
      client.destroyNexus(args, (err, data) => {
        if (err) return reject(err);
        resolve(data);
      });
    });
  };

  const createNexus = (args) => {
    return new Promise((resolve, reject) => {
      client.createNexus(args, (err, data) => {
        if (err) return reject(err);
        resolve(data);
      });
    });
  };

  const createNexusV2 = (args) => {
    return new Promise((resolve, reject) => {
      client.createNexusV2(args, (err, data) => {
        if (err) return reject(err);
        resolve(data);
      });
    });
  };

  const createArgs = {
    uuid: UUID,
    size: 131072,
    children: [
      `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`,
      `aio://${aioFile}?blk_size=4096`
    ]
  };
  this.timeout(50000); // for network tests we need long timeouts

  before((done) => {
    client = common.createGrpcClient();
    if (!client) {
      return done(new Error('Failed to initialize grpc client'));
    }

    async.series(
      [
        common.ensureNbdWritable,
        // start this as early as possible to avoid mayastor getting connection refused.
        (next) => {
          // Start two Mayastor instances. The first one will hold the remote
          // nvmf target and the second one everything including nexus.
          // We must do this because if nvme initiator and target are in
          // the same instance, the SPDK will hang.
          //
          // In order not to exceed available memory in hugepages when running
          // two instances we use the -s option to limit allocated mem.
          common.startMayastor(configNvmfTarget, [
            '-r',
            '/tmp/target.sock',
            '-s',
            '128',
            '-g',
            '127.0.0.1:10125'
          ],
          { MY_POD_IP: '127.0.0.1' },
          '_tgt');
          common.waitFor((pingDone) => {
            // use harmless method to test if spdk is up and running
            common.jsonrpcCommand('/tmp/target.sock', 'bdev_get_bdevs', pingDone);
          }, next);
        },
        (next) => {
          common.createBdevs([`malloc:///Malloc0?size_mb=64&blk_size=4096&uuid=${TGTUUID}`], 'nvmf', '127.0.0.1:10125', next);
        },
        (next) => {
          fs.writeFile(aioFile, '', next);
        },
        (next) => {
          fs.truncate(aioFile, diskSize, next);
        },
        (next) => {
          fs.writeFile(uringFile, '', next);
        },
        (next) => {
          fs.truncate(uringFile, diskSize, next);
        },
        (next) => {
          if (doUring()) { createArgs.children.push(`uring://${uringFile}?blk_size=4096`); }
          next();
        },
        (next) => {
          common.startMayastor(null, ['-r', common.SOCK, '-g', common.grpcEndpoint, '-s', 384],
            { NEXUS_NVMF_ANA_ENABLE: '1' });

          common.waitFor((pingDone) => {
            // use harmless method to test if the mayastor is up and running
            client.listPools({}, pingDone);
          }, next);
        },
        (next) => {
          common.createBdevs(['malloc:///Malloc0?size_mb=64&blk_size=4096'], 'nvmf', common.grpcEndpoint, next);
        }
      ],
      done
    );
  });

  after((done) => {
    async.series(
      [
        common.stopAll,
        common.restoreNbdPerms,
        (next) => {
          fs.unlink(aioFile, (err) => {
            if (err) console.log('unlink failed:', aioFile, err);
            next();
          });
        },
        (next) => {
          if (doUring()) {
            fs.unlink(uringFile, (err) => {
              if (err) console.log('unlink failed:', uringFile, err);
              next();
            });
          } else next();
        }
      ],
      (err) => {
        if (client != null) {
          client.close();
        }
        done(err);
      }
    );
  });

  function createNexusWithAllTypes (done) {
    const args = {
      uuid: UUID,
      size: diskSize,
      children: [
        'bdev:///Malloc0',
        `aio://${aioFile}?blk_size=4096`,
        `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`
      ]
    };
    if (doUring()) args.children.push(`uring://${uringFile}?blk_size=4096`);

    client.createNexus(args, done);
  }

  it('should create a nexus using all types of replicas', (done) => {
    createNexusWithAllTypes((err, nexus) => {
      if (err) return done(err);
      const expectedChildren = 3 + doUring();
      assert.equal(nexus.uuid, UUID);
      assert.equal(nexus.state, 'NEXUS_ONLINE');
      assert.lengthOf(nexus.children, expectedChildren);
      assert.equal(nexus.children[0].uri, 'bdev:///Malloc0');
      assert.equal(nexus.children[0].state, 'CHILD_ONLINE');
      assert.equal(nexus.children[1].uri, `aio://${aioFile}?blk_size=4096`);
      assert.equal(nexus.children[1].state, 'CHILD_ONLINE');

      assert.equal(
        nexus.children[2].uri,
        `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`
      );
      assert.equal(nexus.children[2].state, 'CHILD_ONLINE');

      if (doUring()) {
        assert.equal(
          nexus.children[3].uri,
          `uring://${uringFile}?blk_size=4096`
        );
        assert.equal(nexus.children[3].state, 'CHILD_ONLINE');
      }
      done();
    });
  });

  it('should succeed if creating the same nexus that already exists', (done) => {
    createNexusWithAllTypes((err, nexus) => {
      if (err) return done(err);
      assert.equal(nexus.uuid, UUID);
      assert.equal(nexus.state, 'NEXUS_ONLINE');
      done();
    });
  });

  it('should list the created nexus', (done) => {
    client.listNexus({}, (err, res) => {
      if (err) return done(err);
      assert.lengthOf(res.nexus_list, 1);

      const nexus = res.nexus_list[0];
      const expectedChildren = 3 + doUring();

      assert.equal(nexus.uuid, UUID);
      assert.equal(nexus.state, 'NEXUS_ONLINE');
      assert.lengthOf(nexus.children, expectedChildren);
      assert.equal(nexus.children[0].uri, 'bdev:///Malloc0');
      assert.equal(nexus.children[0].state, 'CHILD_ONLINE');
      assert.equal(nexus.children[1].uri, `aio://${aioFile}?blk_size=4096`);
      assert.equal(nexus.children[1].state, 'CHILD_ONLINE');

      assert.equal(
        nexus.children[2].uri,
        `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`
      );
      assert.equal(nexus.children[2].state, 'CHILD_ONLINE');

      if (doUring()) {
        assert.equal(
          nexus.children[3].uri,
          `uring://${uringFile}?blk_size=4096`
        );
        assert.equal(nexus.children[3].state, 'CHILD_ONLINE');
      }
      done();
    });
  });

  it('should be able to remove one of its children', (done) => {
    const args = {
      uuid: UUID,
      uri: `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`
    };

    client.removeChildNexus(args, (err) => {
      if (err) return done(err);

      client.listNexus({}, (err, res) => {
        if (err) return done(err);
        const nexus = res.nexus_list[0];
        const expectedChildren = 2 + doUring();
        assert.lengthOf(nexus.children, expectedChildren);
        assert(!nexus.children.find((ch) => ch.uri.match(/^nvmf:/)));
        done();
      });
    });
  });

  it('should be able to add the child back', (done) => {
    const uri = `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`;
    const args = {
      uuid: UUID,
      uri: uri,
      norebuild: false
    };

    client.addChildNexus(args, (err, res) => {
      if (err) return done(err);
      assert.equal(res.uri, uri);
      assert.equal(res.state, 'CHILD_DEGRADED');

      client.listNexus({}, (err, res) => {
        if (err) return done(err);
        const nexus = res.nexus_list[0];
        const expectedChildren = 3 + doUring();
        assert.lengthOf(nexus.children, expectedChildren);
        assert(nexus.children.find((ch) => ch.uri.match(/^nvmf:/)));
        done();
      });
    });
  });

  it('should fail to create another nexus with in use URIs', (done) => {
    const args = {
      uuid: UUID2,
      size: 131072,
      children: [`nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`]
    };

    client.createNexus(args, (err) => {
      if (!err) return done(new Error('Expected error'));
      assert.equal(err.code, grpc.status.INVALID_ARGUMENT);
      done();
    });
  });

  it('should fail creating a nexus with non existing URIs', (done) => {
    const args = {
      uuid: UUID2,
      size: 131072,
      children: [
        'nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:disk3'
      ]
    };

    client.createNexus(args, (err) => {
      assert.equal(err.code, grpc.status.INVALID_ARGUMENT);
      done();
    });
  });

  describe.skip('nbd control', function () {
    controlPlaneTest(enums.NEXUS_NBD);
  }); // End describe('nbd control')

  describe.skip('nbd datapath', function () {
    let nbdDeviceUri;

    it('should publish the nexus', (done) => {
      client.publishNexus(
        {
          uuid: UUID,
          share: enums.NEXUS_NBD
        },
        (err, res) => {
          if (err) done(err);
          assert(res.device_uri);
          nbdDeviceUri = res.device_uri;
          done();
        }
      );
    });

    it('should be able to write to the NBD device', async () => {
      const fs = require('fs').promises;
      const deviceURL = new url.URL(nbdDeviceUri);
      const fd = await fs.open(deviceURL.pathname, 'w', 666);
      const buffer = Buffer.alloc(512, 'z', 'utf8');
      await fd.write(buffer, 0, 512);
      await fd.sync();
      await fd.close();
    });

    it('should be able to read the written data back', async () => {
      const fs = require('fs').promises;
      const deviceURL = new url.URL(nbdDeviceUri);
      const fd = await fs.open(deviceURL.pathname, 'r', 666);
      const buffer = Buffer.alloc(512, 'a', 'utf8');
      await fd.read(buffer, 0, 512);
      await fd.close();

      buffer.forEach((e) => {
        assert(e === 122);
      });
    });

    it('should un-publish the NBD nexus device', (done) => {
      client.unpublishNexus({ uuid: UUID }, (err, res) => {
        if (err) done(err);
        done();
      });
    });
  }); // End describe('nbd datapath')

  describe('nvmf control', function () {
    controlPlaneTest(enums.NEXUS_NVMF);
  }); // End describe('nvmf control')

  describe('nvmf datapath', function () {
    const blockFile = '/tmp/test_block';
    const idCtrlrFile = '/tmp/nvme-id-ctrlr';

    function rmBlockFile (done) {
      common.execAsRoot('rm', ['-f', blockFile], () => {
        // ignore unlink error
        done();
      });
    }

    before((done) => {
      const buf = Buffer.alloc(4096, 'm');

      async.series(
        [
          (next) => rmBlockFile(next),
          (next) => fs.writeFile(blockFile, buf, next)
        ],
        done
      );
    });

    let uri;
    it('should publish the nexus', (done) => {
      client.publishNexus(
        {
          uuid: UUID,
          share: enums.NEXUS_NVMF
        },
        (err, res) => {
          if (err) done(err);
          assert(res.device_uri);
          uri = res.device_uri;
          done();
        }
      );
    });

    it('should discover the nvmf nexus device', (done) => {
      common.execAsRoot('nvme', ['discover', '-a', externIp, '-s', '8420', '-t', 'tcp', '-q', 'nqn.2014-08.org.nvmexpress.discovery'], (err, stdout) => {
        if (err) {
          done(err);
        } else {
          // The discovery reply text should contain our nexus
          assert.include(stdout.toString(), 'nqn.2019-05.io.openebs:nexus-' + UUID);
          done();
        }
      });
    });

    // technically control path but this is nvmf-only
    it('should identify nvmf controller', (done) => {
      common.execAsRoot(common.getCmdPath('initiator'), [uri, 'id-ctrlr', idCtrlrFile], (err, stdout) => {
        if (err) {
          done(err);
        } else {
          fs.readFile(idCtrlrFile, (err, data) => {
            if (err) throw err;
            // Identify Controller Data Structure
            // nvme_id_ctrl or spdk_nvme_ctrlr_data
            assert.equal(data.length, 4096);
            // model number
            assert.equal(data.slice(24, 32).toString(), 'Mayastor');
            // cmic, bit 3 ana_reporting
            assert.equal((data[76] & 0x8), 0x8, 'ANA reporting should be enabled');
            // cntlid
            const cntlid = data[79] << 8 | data[78];
            assert.equal(cntlid, 1, 'should match expected cntlid');
          });
          done();
        }
      });
    });

    it('should change ANA state to inaccessible', (done) => {
      client.setNvmeAnaState(
        {
          uuid: UUID,
          ana_state: enums.NVME_ANA_INACCESSIBLE_STATE
        },
        done
      );
    });

    it('should get ANA state as inaccessible', (done) => {
      client.getNvmeAnaState({ uuid: UUID }, (err, res) => {
        if (err) done(err);
        assert.equal(res.ana_state, 'NVME_ANA_INACCESSIBLE_STATE');
        done();
      });
    });

    it('should change ANA state back to optimized', (done) => {
      client.setNvmeAnaState(
        {
          uuid: UUID,
          ana_state: enums.NVME_ANA_OPTIMIZED_STATE
        },
        done
      );
    });

    it('should get ANA state as optimized', (done) => {
      client.getNvmeAnaState({ uuid: UUID }, (err, res) => {
        if (err) done(err);
        assert.equal(res.ana_state, 'NVME_ANA_OPTIMIZED_STATE');
        done();
      });
    });

    it('should write to nvmf replica', (done) => {
      common.execAsRoot(
        common.getCmdPath('initiator'),
        ['--offset=4096', uri, 'write', blockFile],
        done
      );
    });

    it('should un-publish the nvmf nexus device', (done) => {
      client.unpublishNexus({ uuid: UUID }, done);
    });

    it('should share the nexus bdev over nvmf with controller ID range', (done) => {
      const args = {
        name: `nexus-${UUID}`,
        protocol: 'nvmf',
        cntlid_min: 1234,
        cntlid_max: 1250
      };
      common.jsonrpcCommand(null, 'nexus_share', args, done);
    });

    it('nvmf controller should have expected controller ID', (done) => {
      common.execAsRoot(common.getCmdPath('initiator'), [uri, 'id-ctrlr', idCtrlrFile], (err, stdout) => {
        if (err) {
          done(err);
        } else {
          fs.readFile(idCtrlrFile, (err, data) => {
            if (err) throw err;
            // Identify Controller Data Structure
            // nvme_id_ctrl or spdk_nvme_ctrlr_data
            assert.equal(data.length, 4096);
            // cntlid
            const cntlid = data[79] << 8 | data[78];
            assert.equal(cntlid, 1234, 'should match expected cntlid');
          });
          done();
        }
      });
    });
  }); // End of describe('nvmf datapath')

  describe('destructive', function () {
    it('should destroy the nexus without explicitly un-publishing it', (done) => {
      client.destroyNexus({ uuid: UUID }, (err) => {
        if (err) return done(err);

        client.listNexus({}, (err, res) => {
          if (err) return done(err);
          assert.lengthOf(res.nexus_list, 0);
          done();
        });
      });
    });

    it('should fail to create a nexus with mixed block sizes', (done) => {
      const args = {
        uuid: UUID,
        size: 131072,
        children: [
          'malloc:///malloc1?size_mb=64',
        `aio://${aioFile}?blk_size=4096`
        ]
      };
      client.createNexus(args, (err) => {
        if (!err) return done(new Error('Expected error'));
        assert.equal(err.code, grpc.status.INVALID_ARGUMENT);
        done();
      });
    });

    it('should fail to create a nexus with size larger than any of its replicas', (done) => {
      const args = {
        uuid: UUID,
        size: 2 * diskSize,
        children: [
        `aio://${aioFile}?blk_size=4096`,
        `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`
        ]
      };

      client.createNexus(args, (err) => {
        if (!err) return done(new Error('Expected error'));
        // todo: fixme
        // in this case we hit a Error::NexusCreate which atm is converted
        // into a grpc internal error
        assert.equal(err.code, grpc.status.INTERNAL);
        done();
      });
    });

    it('should create v2 and destroy a nexus with UUID as name', async () => {
      const args = {
        name: NEXUSUUID,
        uuid: UUID,
        size: 32768,
        minCntlId: 1,
        maxCntlId: 0xffef,
        resvKey: 0x12345678,
        children: [
          'malloc:///malloc1?size_mb=64'
        ]
      };
      await createNexusV2(args);
      await destroyNexus({ uuid: NEXUSUUID });
    });

    it('should fail to create a nexus with invalid NVMe controller ID range', (done) => {
      const args = {
        name: NEXUSNAME,
        uuid: UUID,
        size: 131072,
        minCntlId: 0xfff0,
        maxCntlId: 1,
        resvKey: 0x12345678,
        children: [
          'malloc:///malloc1?size_mb=64'
        ]
      };
      client.createNexusV2(args, (err) => {
        if (!err) return done(new Error('Expected error'));
        assert.equal(err.code, grpc.status.INTERNAL);
        done();
      });
    });

    it('should fail to create a nexus with invalid NVMe reservation key', (done) => {
      const args = {
        name: NEXUSNAME,
        uuid: UUID,
        size: 131072,
        minCntlId: 1,
        maxCntlId: 0xffef,
        resvKey: 0,
        preemptKey: 1,
        children: [
          'malloc:///malloc1?size_mb=64'
        ]
      };
      client.createNexusV2(args, (err) => {
        if (!err) return done(new Error('Expected error'));
        assert.equal(err.code, grpc.status.INTERNAL);
        done();
      });
    });

    it('should have zero nexus devices left', (done) => {
      client.listNexus({}, (err, res) => {
        if (err) return done(err);
        assert.lengthOf(res.nexus_list, 0);
        done();
      });
    });

    it.skip('should create, publish, un-publish and finally destroy the same NBD nexus', async () => {
      for (let i = 0; i < 10; i++) {
        await createNexus(createArgs);
        await publish({
          uuid: UUID,
          share: enums.NEXUS_NBD
        });
        await unpublish({ uuid: UUID });
        await destroyNexus({ uuid: UUID });
      }
    });

    it('should have zero nexus devices left', (done) => {
      client.listNexus({}, (err, res) => {
        if (err) return done(err);
        assert.lengthOf(res.nexus_list, 0);
        done();
      });
    });

    it.skip('should create, publish, and destroy but without un-publishing the same nexus, with NBD protocol', async () => {
      for (let i = 0; i < 10; i++) {
        await createNexus(createArgs);
        await publish({
          uuid: UUID,
          share: enums.NEXUS_NBD
        });
        await destroyNexus({ uuid: UUID });
      }
    });

    it('should have zero nexus devices left', (done) => {
      client.listNexus({}, (err, res) => {
        if (err) return done(err);
        assert.lengthOf(res.nexus_list, 0);
        done();
      });
    });

    it('should create and destroy without publish or un-publishing the same nexus', async () => {
      for (let i = 0; i < 10; i++) {
        await createNexus(createArgs);
        await destroyNexus({ uuid: UUID });
      }
    });

    it('should create nexus with nvmf target as child', async () => {
      const args = {
        uuid: UUID,
        size: diskSize,
        children: [
        `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`
        ]
      };
      await createNexus(args);
    });

    it('should remove namespace from nvmf subsystem', (done) => {
      const args = {
        nqn: `nqn.2019-05.io.openebs:${BASEDEV}`,
        nsid: 1
      };
      common.jsonrpcCommand('/tmp/target.sock', 'nvmf_subsystem_remove_ns', args, done);
    });

    it('dummy call to get list of bdevs to allow remove event to be processed', (done) => {
      common.jsonrpcCommand(null, 'bdev_get_bdevs', (err, out) => {
        if (err) return done(err);
        const bdevs = JSON.parse(out);
        const match = `127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}n1`;
        let i;
        for (i in bdevs) {
          if (bdevs[i].name === match) {
            return done();
          }
        }
        done();
      });
    });

    it('should have nexus in faulted state and its child in degraded state', (done) => {
      client.listNexus({}, (err, res) => {
        if (err) return done(err);
        assert.lengthOf(res.nexus_list, 1);
        const nexus = res.nexus_list[0];

        assert.equal(nexus.uuid, UUID);
        assert.equal(nexus.state, 'NEXUS_FAULTED');
        assert.lengthOf(nexus.children, 1);
        assert.equal(nexus.children[0].state, 'CHILD_DEGRADED');
        done();
      });
    });

    it('should destroy nexus', async () => {
      await destroyNexus({ uuid: UUID });
    });

    it('should fail to create nexus with child that has no namespaces', (done) => {
      const args = {
        uuid: UUID,
        size: diskSize,
        children: [
        `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`
        ]
      };

      client.createNexus(args, (err) => {
        if (!err) return done(new Error('Expected error'));
        assert.equal(err.code, grpc.status.INVALID_ARGUMENT);
        done();
      });
    });

    it('should add namespace back to nvmf subsystem', (done) => {
      const args = {
        nqn: `nqn.2019-05.io.openebs:${BASEDEV}`,
        namespace: {
          bdev_name: 'Malloc0'
        }
      };
      common.jsonrpcCommand('/tmp/target.sock', 'nvmf_subsystem_add_ns', args, done);
    });

    it('should create then destroy nexus with previously asynchronously removed nvmf target as child', async () => {
      const args = {
        uuid: UUID,
        size: diskSize,
        children: [
        `nvmf://127.0.0.1:8420/nqn.2019-05.io.openebs:${BASEDEV}`
        ]
      };
      await createNexus(args);
      await destroyNexus({ uuid: UUID });
    });

    it('should have zero nexus devices left', (done) => {
      client.listNexus({}, (err, res) => {
        if (err) return done(err);
        assert.lengthOf(res.nexus_list, 0);
        done();
      });
    });

    it.skip('should be the case that we do not have any dangling NBD devices left on the system', (done) => {
      exec('sleep 3; lsblk --json', (err, stdout, stderr) => {
        if (err) return done(err);
        const output = JSON.parse(stdout);
        output.blockdevices.forEach((e) => {
          assert(e.name.indexOf('nbd') === -1, `NBD Device found:\n${stdout}`);
        });
        done();
      });
    });
  }); // End of describe('destructive')
});
