dirs = [
    'accelchain',
    'bench-apps',
    'cppbenchs',
    'cppnetbenchs',
    'distinf',
    'facever',
    'fs',
    'fstrace',
    'hashmuxbenchs',
    'imgproc',
    'ipc',
    'ripc',
    'bencrmgate',
    'bmgatelarge',
    'bp2pspm',
    'tracereplay',
    'encrsyscall',
    'loadgen',
    'netlat',
    'rustbenchs',
    'rustnetbenchs',
    'scale',
    'scale-pipe',
    'tlbmiss',
    'voiceassist',
    'ycsb',
]

def build(gen, env):
    for d in dirs:
        env.sub_build(gen, d)
