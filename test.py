from math import sin, cos

functions = [
    ["sin(t)", sin],
    ["cos(t)", cos],
    ["sin(t)%0.9", lambda t: sin(t)%0.9]
]
print(",".join(list(map((lambda f: "\""+f[0]+"\""), functions))))
for t in range(10000):
    t /= 7
    print(",".join(list(map((lambda f: str(f[1](t))), functions))))
