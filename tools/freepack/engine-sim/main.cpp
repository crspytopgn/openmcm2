// Headless engine-sim renderer: es-render <script.mr> <out.wav> <schedule.txt>
// schedule lines: seconds throttle dyno_rpm gear(-1 = neutral)
#include "include/engine.h"
#include "include/simulator.h"
#include "include/transmission.h"
#include "include/vehicle.h"
#include "include/units.h"
#include "include/exhaust_system.h"
#include "include/impulse_response.h"
#include "include/ignition_module.h"
#include "scripting/include/compiler.h"
#include <cstdio>
#include <cstring>
#include <vector>
#include <fstream>
#include <thread>
#include <chrono>

static std::vector<int16_t> readWav(const std::string &fn) {
    std::ifstream f(fn, std::ios::binary);
    std::vector<char> d((std::istreambuf_iterator<char>(f)), {});
    std::vector<int16_t> out;
    size_t p = 12; int ch = 1, bits = 16;
    while (p + 8 <= d.size()) {
        uint32_t sz; memcpy(&sz, &d[p + 4], 4);
        if (!memcmp(&d[p], "fmt ", 4)) { ch = *(int16_t *)&d[p + 10]; bits = *(int16_t *)&d[p + 22]; }
        if (!memcmp(&d[p], "data", 4)) {
            size_t n = sz / (bits / 8) / ch;
            for (size_t i = 0; i < n; ++i) {
                if (bits == 16) out.push_back(*(int16_t *)&d[p + 8 + i * 2 * ch]);
                else if (bits == 24) { int v = (uint8_t)d[p+8+i*3*ch] | ((uint8_t)d[p+9+i*3*ch] << 8) | ((int8_t)d[p+10+i*3*ch] << 16); out.push_back(v >> 8); }
            }
            break;
        }
        p += 8 + sz + (sz & 1);
    }
    return out;
}

extern std::vector<float> g_floatOut;
int main(int argc, char **argv) {
    if (argc < 4) { fprintf(stderr, "usage\n"); return 1; }
    es_script::Compiler compiler;
    compiler.initialize();
    if (!compiler.compile(argv[1])) { fprintf(stderr, "compile failed (see error_log.log)\n"); return 1; }
    auto output = compiler.execute();
    Engine *engine = output.engine; Vehicle *vehicle = output.vehicle; Transmission *transmission = output.transmission;
    compiler.destroy();
    if (!engine) { fprintf(stderr, "no engine\n"); return 1; }
    if (!vehicle) {
        Vehicle::Parameters vp; vp.mass = units::mass(200, units::kg); vp.diffRatio = 3.0;
        vp.tireRadius = units::distance(10, units::inch); vp.dragCoefficient = 0.6;
        vp.crossSectionArea = units::distance(3.0, units::foot) * units::distance(3.0, units::foot);
        vp.rollingResistance = 200.0; vehicle = new Vehicle; vehicle->initialize(vp);
    }
    if (!transmission) {
        static const double gr[] = {2.5, 1.9, 1.5, 1.2, 1.0};
        Transmission::Parameters tp; tp.GearCount = 5; tp.GearRatios = gr; tp.MaxClutchTorque = units::torque(200.0, units::ft_lb);
        transmission = new Transmission; transmission->initialize(tp);
    }
    engine->calculateDisplacement();
    Simulator *sim = engine->createSimulator(vehicle, transmission);
    sim->setSimulationFrequency(engine->getSimulationFrequency());
    auto ap = sim->synthesizer().getAudioParameters();
    ap.inputSampleNoise = (float)engine->getInitialJitter();
    ap.airNoise = (float)engine->getInitialNoise();
    ap.dF_F_mix = (float)engine->getInitialHighFrequencyGain();
    if (getenv("GAIN")) { ap.levelerMinGain = ap.levelerMaxGain = atof(getenv("GAIN")); ap.levelerTarget = 1e9f; } else ap.levelerTarget = 9000.0f;
    sim->synthesizer().setAudioParameters(ap);
    for (int i = 0; i < engine->getExhaustSystemCount(); ++i) {
        ImpulseResponse *r = engine->getExhaustSystem(i)->getImpulseResponse();
        auto w = readWav(r->getFilename());
        fprintf(stderr, "IR %s: %zu samples\n", r->getFilename().c_str(), w.size());
        sim->synthesizer().initializeImpulseResponse(w.data(), (unsigned)w.size(), r->getVolume(), i);
    }
    sim->startAudioRenderingThread();

    struct Step { double t, thr, dyno, dyno1; int gear; };
    std::vector<Step> sched; { FILE *f = fopen(argv[3], "r"); Step s; while (f && fscanf(f, "%lf %lf %lf %lf %d", &s.t, &s.thr, &s.dyno, &s.dyno1, &s.gear) == 5) sched.push_back(s); }
    std::vector<int16_t> audio;
    const double dt = 1.0 / 120.0;
    double speed = 0.0; double clutch = 0.0;
    engine->getIgnitionModule()->m_enabled = true;
    double t = 0; size_t si = 0; double segStart = 0; double nextLog = 0;
    double total = 0; for (auto &s : sched) total += s.t;
    sim->m_starterMotor.m_enabled = true;
    transmission->changeGear(-1);
    int16_t buf[44100];
    while (t < total) {
        while (si < sched.size() && t >= segStart + sched[si].t) { segStart += sched[si].t; ++si; }
        if (si >= sched.size()) break;
        const Step &s = sched[si];
        if (t > 1.0) sim->m_starterMotor.m_enabled = false;
        speed = s.thr * 0.15 + speed * 0.85;
        engine->setSpeedControl(speed);
        if (transmission->getGear() != s.gear) {
            if (transmission->getGear() >= 0 && s.gear >= 0) clutch = 0.4;
            transmission->changeGear(s.gear);
        }
        {
            double target = s.gear < 0 ? 0.0 : 1.0;
            double rate = clutch < 0.6 ? 1.2 : 4.0;
            if (clutch < target) clutch = std::min(target, clutch + rate * dt);
            else clutch = target;
            transmission->setClutchPressure(clutch);
        }
        sim->m_dyno.m_enabled = s.dyno > 0; sim->m_dyno.m_hold = s.dyno > 0;
        { double a = (t - segStart) / s.t; sim->m_dyno.m_rotationSpeed = units::rpm(s.dyno + (s.dyno1 - s.dyno) * a); }
        sim->startFrame(dt);
        while (sim->simulateStep()) {}
        sim->endFrame();
        int frameSteps = sim->simulationSteps();
        t += frameSteps / (double)sim->getSimulationFrequency();
        sim->synthesizer().waitProcessed();
        int n;
        while ((n = sim->readAudioOutput(44100, buf)) > 0) audio.insert(audio.end(), buf, buf + n);
        if (t >= nextLog) { fprintf(stderr, "t=%.2f rpm=%.0f thr=%.2f gear=%d clutch=%.2f v=%.1f audio=%.2fs\n", t, engine->getRpm(), s.thr, transmission->getGear(), clutch, vehicle->getSpeed(), audio.size() / 44100.0); nextLog += getenv("LOGI") ? atof(getenv("LOGI")) : 0.5; }
    }
    sim->endAudioRenderingThread();
    FILE *o = fopen(argv[2], "wb");
    uint32_t ds = audio.size() * 2, rs = 36 + ds, sr = 44100, br = 88200; uint16_t one = 1, ba = 2, bps = 16; uint32_t f16 = 16;
    fwrite("RIFF", 1, 4, o); fwrite(&rs, 4, 1, o); fwrite("WAVEfmt ", 1, 8, o); fwrite(&f16, 4, 1, o);
    fwrite(&one, 2, 1, o); fwrite(&one, 2, 1, o); fwrite(&sr, 4, 1, o); fwrite(&br, 4, 1, o); fwrite(&ba, 2, 1, o); fwrite(&bps, 2, 1, o);
    fwrite("data", 1, 4, o); fwrite(&ds, 4, 1, o); fwrite(audio.data(), 2, audio.size(), o); fclose(o);
    { std::string fn = std::string(argv[2]) + ".f32"; FILE *g = fopen(fn.c_str(), "wb"); fwrite(g_floatOut.data(), 4, g_floatOut.size(), g); fclose(g); }
    fprintf(stderr, "wrote %.2fs\n", audio.size() / 44100.0);
    return 0;
}
