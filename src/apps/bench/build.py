dirs = [
    'accelchain',
    'bench-apps',
    'cppbenchs',
    'cppnetbenchs',
    'facever',
    'fs',
    'fstrace',
    'hashmuxbenchs',
    'imgproc',
    'ipc',
    'ripc',
    'bencrmgate',
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
