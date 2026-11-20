drop_in = "/etc/systemd/sleep.conf.d/50-sottomano.conf"


def start_as(user_name, instance_name):
    return f"su {user_name} -c 'systemctl start sottomano-hibernate-delay@{instance_name}.service'"


machine.wait_for_unit("multi-user.target")

machine.succeed(start_as("alice", "3600"))
machine.succeed(f"grep -Fx 'HibernateDelaySec=3600s' {drop_in}")
machine.succeed("systemd-analyze cat-config systemd/sleep.conf | grep -Fx 'HibernateDelaySec=3600s'")

machine.fail(start_as("alice", "0600"))
machine.fail(start_as("alice", "abc"))
machine.fail(start_as("alice", "999999"))
machine.fail(start_as("bob", "1800"))

machine.succeed(f"grep -Fx 'HibernateDelaySec=3600s' {drop_in}")

machine.succeed(start_as("alice", "10800"))
machine.succeed(f"grep -Fx 'HibernateDelaySec=10800s' {drop_in}")
