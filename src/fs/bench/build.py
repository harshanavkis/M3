def build(gen, env):
    if env['TGT'] == 'host':
        blocks = 160 * 1024
    else:
        blocks = 64 * 1024
    env.build_fs(gen, out = 'bench.img', dir = '.', blocks = blocks, inodes = 4096)
