from urllib.parse import urlparse
import subprocess
import time
import json
import re
from common.command import run_cmd_async_at


async def nvme_remote_connect_all(remote, host, port):
    command = f"nix-sudo nvme connect-all -t tcp -s {port} -a {host}"
    await run_cmd_async_at(remote, command)


async def nvme_remote_connect(remote, uri):
    """Connect to the remote nvmf target on this host."""
    u = urlparse(uri)
    port = u.port
    host = u.hostname
    nqn = u.path[1:]

    command = "nix-sudo nvme connect -t tcp -s {0} -a {1} -n {2}".format(
        port, host, nqn
    )

    await run_cmd_async_at(remote, command)
    time.sleep(1)
    command = "nix-sudo nvme list -v -o json"

    discover = await run_cmd_async_at(remote, command)
    discover = json.loads(discover.stdout)

    dev = list(filter(lambda d: nqn in d.get("SubsystemNQN"), discover.get("Devices")))

    # we should only have one connection
    assert len(dev) == 1
    dev_path = dev[0].get("Controllers")[0].get("Namespaces")[0].get("NameSpace")

    return f"/dev/{dev_path}"


async def nvme_remote_disconnect(remote, uri):
    """Disconnect the given URI on this host."""
    u = urlparse(uri)
    nqn = u.path[1:]

    command = "nix-sudo nvme disconnect -n {0}".format(nqn)
    await run_cmd_async_at(remote, command)


async def nvme_remote_discover(remote, uri):
    """Discover target."""
    u = urlparse(uri)
    port = u.port
    host = u.hostname

    command = "nix-sudo nvme discover -t tcp -s {0} -a {1}".format(port, host)
    output = await run_cmd_async_at(remote, command).stdout
    if not u.path[1:] in str(output.stdout):
        raise ValueError("uri {} is not discovered".format(u.path[1:]))


def nvme_connect(uri, delay=10, tmo=600):
    u = urlparse(uri)
    port = u.port
    host = u.hostname
    nqn = u.path[1:]

    command = (
        f"nix-sudo nvme connect -t tcp -s {port} -a {host} -n {nqn} -c {delay} -l {tmo}"
    )
    print(command)
    subprocess.run(command, check=True, shell=True, capture_output=False)
    time.sleep(1)
    command = "nix-sudo nvme list -v -o json"
    discover = json.loads(
        subprocess.run(
            command, shell=True, check=True, text=True, capture_output=True
        ).stdout
    )

    dev = list(filter(lambda d: nqn in d.get("SubsystemNQN"), discover.get("Devices")))

    # we should only have one connection
    assert len(dev) == 1
    device = "/dev/{}".format(dev[0]["Namespaces"][0].get("NameSpace"))
    return device


def nvme_id_ctrl(device):
    """Identify controller."""
    command = "nix-sudo nvme id-ctrl {0} -o json".format(device)
    id_ctrl = json.loads(
        subprocess.run(
            command, shell=True, check=True, text=True, capture_output=True
        ).stdout
    )

    return id_ctrl


def match_host_port(addr, host, port):
    traddr = f"traddr={host}"
    trsvcid = f"trsvcid={port}"

    return traddr in addr and trsvcid in addr


def nvme_find_ctrl(uri):
    """Find controller from the device uri."""
    u = urlparse(uri)
    port = u.port
    host = u.hostname
    nqn = u.path[1:]

    command = "nix-sudo nvme list -v -o json"
    discover = json.loads(
        subprocess.run(
            command, shell=True, check=True, text=True, capture_output=True
        ).stdout
    )

    # Finds correct Device
    devs = list(filter(lambda d: nqn in d.get("SubsystemNQN"), discover.get("Devices")))
    assert len(devs) is 1, "Multiple devices with the same subnqn"

    # Find correct Controller
    ctrls = list(
        filter(
            lambda d: match_host_port(d.get("Address"), host, port),
            devs[0].get("Controllers"),
        )
    )
    assert len(ctrls) is 1, "Multiple controllers with the same address"

    return ctrls[0].get("Controller")


def nvme_resv_report(device):
    """Reservation report."""
    command = "nix-sudo nvme resv-report {0} -c 1 -o json".format(device)
    resv_report = json.loads(
        subprocess.run(
            command, shell=True, check=True, text=True, capture_output=True
        ).stdout
    )

    return resv_report


def nvme_discover(uri):
    """Discover target."""
    u = urlparse(uri)
    port = u.port
    host = u.hostname

    command = "nix-sudo nvme discover -t tcp -s {0} -a {1}".format(port, host)
    output = subprocess.run(
        command, check=True, shell=True, capture_output=True, encoding="utf-8"
    )
    if not u.path[1:] in str(output.stdout):
        raise ValueError("uri {} is not discovered".format(u.path[1:]))


def nvme_disconnect(uri):
    """Disconnect the given URI on this host."""
    u = urlparse(uri)
    nqn = u.path[1:]

    command = "nix-sudo nvme disconnect -n {0}".format(nqn)
    print(command)
    subprocess.run(command, check=True, shell=True, capture_output=True)


def nvme_disconnect_controller(name):
    """Disconnect the given NVMe controller on this host."""
    command = "nix-sudo nvme disconnect -d {0}".format(name)
    print(command)
    subprocess.run(command, check=True, shell=True, capture_output=True)


def nvme_disconnect_all():
    """Disconnect from all connected nvme subsystems"""
    command = "nix-sudo nvme disconnect-all"
    print(command)
    subprocess.run(command, check=True, shell=True, capture_output=True)


def nvme_list_subsystems(device):
    """Retrieve information for NVMe subsystems"""
    command = "nix-sudo nvme list-subsys {} -o json".format(device)
    return json.loads(
        subprocess.run(
            command, check=True, shell=True, capture_output=True, encoding="utf-8"
        ).stdout
    )


NS_PROPS = ["nguid", "eui64"]


def identify_namespace(device):
    """Get properties of a namespace on this host"""
    command = "nix-sudo nvme id-ns {}".format(device)
    output = subprocess.run(
        command, check=True, shell=True, capture_output=True, encoding="utf-8"
    )
    props = output.stdout.strip().split("\n")[1:]
    ns = {}
    for p in props:
        v = [v.strip() for v in p.split(":") if p.count(":") == 1]
        if len(v) == 2 and v[0] in NS_PROPS:
            ns[v[0]] = v[1]
    return ns


def nvme_delete_controller(device):
    """Forcibly deletes NVMe controller"""
    # Transparently remove optional global device prefix.
    dev = device.lstrip("/dev/").lstrip()

    # Remove namespace part and leave only controller name.
    r = re.compile(r"(nvme\d+)(n\d+)?")
    m = r.search(dev)
    assert m is not None, "Incorrect NVMe controller name: %s" % device
    p = "/sys/class/nvme/%s/delete_controller" % m.group(1)

    # Forcibly trigger controller removal. Note that operations must be executed
    # with root privileges, hence nix-sudo for python interpreter.
    script = "\"f = open('%s', 'w'); f.write('1'); f.flush()\"" % p
    # Run privileged Python script.
    command = "nix-sudo python -c {} ".format(script)
    subprocess.run(command, check=True, shell=True, capture_output=True)
