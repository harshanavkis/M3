def build(gen, env):
    env.m3_exe(gen, out = 'timer', ins = ['timer.cc'], dir = 'sbin')
