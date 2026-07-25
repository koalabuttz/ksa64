package ksa64.phase8;

import com.google.inject.Guice;
import info.openrocket.core.document.OpenRocketDocument;
import info.openrocket.core.document.OpenRocketDocumentFactory;
import info.openrocket.core.document.Simulation;
import info.openrocket.core.document.StorageOptions;
import info.openrocket.core.file.CSVExport;
import info.openrocket.core.file.openrocket.OpenRocketSaver;
import info.openrocket.core.logging.ErrorSet;
import info.openrocket.core.logging.WarningSet;
import info.openrocket.core.motor.Manufacturer;
import info.openrocket.core.motor.Motor;
import info.openrocket.core.motor.MotorConfiguration;
import info.openrocket.core.motor.ThrustCurveMotor;
import info.openrocket.core.plugin.PluginModule;
import info.openrocket.core.rocketcomponent.AxialStage;
import info.openrocket.core.rocketcomponent.BodyTube;
import info.openrocket.core.rocketcomponent.DeploymentConfiguration;
import info.openrocket.core.rocketcomponent.FlightConfigurationId;
import info.openrocket.core.rocketcomponent.MassComponent;
import info.openrocket.core.rocketcomponent.NoseCone;
import info.openrocket.core.rocketcomponent.Parachute;
import info.openrocket.core.rocketcomponent.Rocket;
import info.openrocket.core.rocketcomponent.RailButton;
import info.openrocket.core.rocketcomponent.TrapezoidFinSet;
import info.openrocket.core.rocketcomponent.Transition;
import info.openrocket.core.rocketcomponent.position.AxialMethod;
import info.openrocket.core.simulation.FlightData;
import info.openrocket.core.simulation.FlightDataBranch;
import info.openrocket.core.simulation.FlightDataType;
import info.openrocket.core.simulation.FlightEvent;
import info.openrocket.core.startup.Application;
import info.openrocket.core.startup.CoreModule;
import info.openrocket.core.unit.Unit;
import info.openrocket.core.util.Coordinate;

import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Locale;

/** OpenRocket 24.12 headless evidence generator for the Phase 8 reference vehicle. */
public final class FirestormEvidence {
    private static final FlightConfigurationId FCID =
            new FlightConfigurationId("be94af9e-7caf-4387-a049-f4aa7894799d");
    private static final double BODY_DIAMETER = 0.057658;
    private static final double NOSE_LENGTH = 0.28829;
    private static final double TOTAL_LENGTH = 1.8923;
    private static final double DRY_MASS = 2.1120394728125;
    private static final double MOTOR_LOADED_MASS = 0.466368;
    private static final double MOTOR_DRY_MASS = 0.219072;
    private static final double MOTOR_LENGTH = 0.35;

    private FirestormEvidence() {}

    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("usage: FirestormEvidence OUTPUT_DIRECTORY");
        }
        Locale.setDefault(Locale.ROOT);
        Path output = Path.of(args[0]);
        Files.createDirectories(output);

        CoreModule core = new CoreModule();
        Application.setInjector(Guice.createInjector(core, new PluginModule()));

        OpenRocketDocument document = makeDocument();
        Simulation calm = addSimulation(document, "KSA64 aligned calm", 0.0);
        Simulation crosswind = addSimulation(document, "KSA64 aligned steady 5 m/s", 5.0);
        calm.simulate();
        crosswind.simulate();

        exportCsv(output.resolve("openrocket-calm-v1.csv"), calm);
        exportCsv(output.resolve("openrocket-crosswind-5mps-v1.csv"), crosswind);
        saveDocument(output.resolve("firestorm54-i211w-v1.ork"), document);
        writeSummary(output.resolve("openrocket-summary-v1.json"), calm, crosswind, document);
    }

    private static OpenRocketDocument makeDocument() {
        OpenRocketDocument document = OpenRocketDocumentFactory.createNewRocket();
        Rocket rocket = document.getRocket();
        rocket.setName("KSA64 Firestorm 54 / AeroTech I211W aligned reference");
        rocket.setDesigner("KSA64 Phase 8 evidence harness");
        rocket.createFlightConfiguration(FCID);
        rocket.setSelectedConfiguration(FCID);

        AxialStage stage = rocket.getStage(0);
        stage.setName("Single stage");

        NoseCone nose = new NoseCone(Transition.Shape.OGIVE, NOSE_LENGTH, BODY_DIAMETER / 2.0);
        nose.setName("Five-caliber tangent-ogive nose");
        nose.setOverrideMass(0.25);
        nose.setMassOverridden(true);
        nose.setOverrideCGX(0.144145);
        nose.setCGOverridden(true);
        stage.addChild(nose);

        BodyTube body = new BodyTube(TOTAL_LENGTH - NOSE_LENGTH, BODY_DIAMETER / 2.0, 0.0015);
        body.setName("Firestorm 54 airframe");
        body.setOverrideMass(0.0);
        body.setMassOverridden(true);
        body.setMotorMount(true);
        body.setMotorOverhang(0.0);
        stage.addChild(body);

        TrapezoidFinSet fins = new TrapezoidFinSet(4, 0.254, 0.127, 0.09, 0.1143);
        fins.setName("Equivalent four-fin set");
        fins.setThickness(0.003175);
        fins.setAxialMethod(AxialMethod.TOP);
        fins.setAxialOffset(1.56 - NOSE_LENGTH);
        fins.setOverrideMass(0.0);
        fins.setMassOverridden(true);
        body.addChild(fins);

        addRailButton(body, "Forward rail guide", TOTAL_LENGTH - 0.8128);
        addRailButton(body, "Aft rail guide", TOTAL_LENGTH - 0.2794);

        addPointMass(body, "Forward airframe mass", 0.45, 0.544145);
        addPointMass(body, "Aft airframe mass", 0.45, 1.34615);
        addPointMass(body, "Avionics bay mass", 0.30, 0.95);
        addPointMass(body, "Forward recovery mass", 0.30, 0.65);
        addPointMass(body, "Aft recovery mass", 0.20, 1.25);
        addPointMass(body, "Fin-can and retainer mass", 0.1620394728125, 1.70);

        Parachute drogue = makeParachute("Drogue", 0.2462598348413839, 0.8);
        drogue.setAxialMethod(AxialMethod.TOP);
        drogue.setAxialOffset(1.25 - NOSE_LENGTH);
        DeploymentConfiguration drogueDeploy = drogue.getDeploymentConfigurations().get(FCID);
        drogueDeploy.setDeployEvent(DeploymentConfiguration.DeployEvent.APOGEE);
        drogueDeploy.setDeployDelay(0.0);
        body.addChild(drogue);

        Parachute main = makeParachute("Main", 0.9850393393655356, 0.8);
        main.setAxialMethod(AxialMethod.TOP);
        main.setAxialOffset(0.65 - NOSE_LENGTH);
        DeploymentConfiguration mainDeploy = main.getDeploymentConfigurations().get(FCID);
        mainDeploy.setDeployEvent(DeploymentConfiguration.DeployEvent.ALTITUDE);
        mainDeploy.setDeployAltitude(200.0);
        mainDeploy.setDeployDelay(0.0);
        body.addChild(main);

        MotorConfiguration motor = new MotorConfiguration(body, FCID);
        motor.setMotor(makeMotor());
        motor.setEjectionDelay(Motor.PLUGGED_DELAY);
        body.setMotorConfig(motor, FCID);

        rocket.enableEvents();
        rocket.update();
        return document;
    }

    private static void addRailButton(BodyTube body, String name, double globalX) {
        RailButton guide = new RailButton();
        guide.setName(name);
        guide.setOuterDiameter(0.012);
        guide.setInnerDiameter(0.006);
        guide.setTotalHeight(0.008);
        guide.setAxialMethod(AxialMethod.TOP);
        guide.setAxialOffset(globalX - NOSE_LENGTH);
        guide.setOverrideMass(0.0);
        guide.setMassOverridden(true);
        body.addChild(guide);
    }
    private static void addPointMass(BodyTube body, String name, double mass, double globalX) {
        MassComponent component = new MassComponent();
        component.setName(name);
        component.setLength(0.001);
        component.setRadius(0.001);
        component.setComponentMass(mass);
        component.setAxialMethod(AxialMethod.TOP);
        component.setAxialOffset(globalX - NOSE_LENGTH - 0.0005);
        body.addChild(component);
    }

    private static Parachute makeParachute(String name, double cda, double cd) {
        Parachute chute = new Parachute();
        chute.setName(name);
        chute.setCDAutomatic(false);
        chute.setCD(cd);
        chute.setDiameter(Math.sqrt(4.0 * cda / (Math.PI * cd)));
        chute.setOverrideMass(0.0);
        chute.setMassOverridden(true);
        return chute;
    }

    private static ThrustCurveMotor makeMotor() {
        double[] time = {0, .044, .134, .226, .318, .408, .499, .591, .682, .773, .864,
                .955, 1.047, 1.138, 1.228, 1.320, 1.411, 1.502, 1.593, 1.684, 1.776,
                1.867, 1.957, 2.049, 2.141, 2.232, 2.324};
        double[] thrust = {0, 257.326, 295.533, 296.087, 298.204, 295.082, 287.669,
                282.578, 272.875, 266.997, 257.602, 250.495, 238.574, 228.571, 215.135,
                198.047, 180.631, 161.261, 146.708, 134.484, 101.241, 52.688, 35.461,
                24.321, 11.165, 4.587, 0};
        Coordinate[] cg = new Coordinate[time.length];
        for (int i = 0; i < time.length; i++) {
            double fraction = time[i] / time[time.length - 1];
            double mass = MOTOR_LOADED_MASS + (MOTOR_DRY_MASS - MOTOR_LOADED_MASS) * fraction;
            double position = 0.175 + (0.18 - 0.175) * fraction;
            cg[i] = new Coordinate(position, 0, 0, mass);
        }
        return new ThrustCurveMotor.Builder()
                .setManufacturer(Manufacturer.getManufacturer("AeroTech"))
                .setDesignation("I211W")
                .setDescription("KSA64 Phase 8 public sampled curve")
                .setCaseInfo("54 mm aligned evidence motor")
                .setMotorType(Motor.Type.RELOAD)
                .setStandardDelays(new double[] {})
                .setDiameter(0.054)
                .setLength(MOTOR_LENGTH)
                .setTimePoints(time)
                .setThrustPoints(thrust)
                .setCGPoints(cg)
                .setDigest("ksa64-aerotech-i211w-spatial-v1")
                .build();
    }

    private static Simulation addSimulation(OpenRocketDocument document, String name, double wind) {
        Simulation simulation = new Simulation(document, document.getRocket());
        simulation.setName(name);
        simulation.setFlightConfigurationId(FCID);
        simulation.getOptions().setISAAtmosphere(true);
        simulation.getOptions().setLaunchAltitude(0.0);
        simulation.getOptions().setLaunchRodLength(2.0 - 0.2794);
        simulation.getOptions().setLaunchRodAngle(0.0);
        simulation.getOptions().setLaunchRodDirection(Math.PI / 2.0);
        simulation.getOptions().setLaunchIntoWind(false);
        simulation.getOptions().setWindSpeedAverage(wind);
        simulation.getOptions().setWindSpeedDeviation(0.0);
        simulation.getOptions().setWindTurbulenceIntensity(0.0);
        simulation.getOptions().setWindDirection(0.0);
        simulation.getOptions().setTimeStep(0.01);
        simulation.getOptions().setMaxSimulationTime(900.0);
        simulation.getOptions().setRandomSeed(0x4b534138);
        document.addSimulation(simulation);
        return simulation;
    }

    private static final FlightDataType[] FIELDS = {
            FlightDataType.TYPE_TIME,
            FlightDataType.TYPE_ALTITUDE,
            FlightDataType.TYPE_POSITION_X,
            FlightDataType.TYPE_POSITION_Y,
            FlightDataType.TYPE_POSITION_XY,
            FlightDataType.TYPE_VELOCITY_TOTAL,
            FlightDataType.TYPE_ACCELERATION_TOTAL,
            FlightDataType.TYPE_MACH_NUMBER,
            FlightDataType.TYPE_AOA,
            FlightDataType.TYPE_PITCH_RATE,
            FlightDataType.TYPE_YAW_RATE,
            FlightDataType.TYPE_MASS,
            FlightDataType.TYPE_CP_LOCATION,
            FlightDataType.TYPE_CG_LOCATION,
            FlightDataType.TYPE_STABILITY,
            FlightDataType.TYPE_DRAG_COEFF,
            FlightDataType.TYPE_THRUST_FORCE,
            FlightDataType.TYPE_AIR_DENSITY,
            FlightDataType.TYPE_WIND_VELOCITY,
            FlightDataType.TYPE_TIME_STEP
    };

    private static void exportCsv(Path path, Simulation simulation) throws IOException {
        Unit[] units = new Unit[FIELDS.length];
        for (int i = 0; i < FIELDS.length; i++) {
            units[i] = FIELDS[i].getUnitGroup().getSIUnit();
        }
        try (FileOutputStream stream = new FileOutputStream(path.toFile())) {
            CSVExport.exportCSV(stream, simulation, simulation.getSimulatedData().getBranch(0),
                    FIELDS, units, ",", 9, false, "#", true, true, true);
        }
    }

    private static void saveDocument(Path path, OpenRocketDocument document) throws IOException {
        StorageOptions options = document.getDefaultStorageOptions();
        options.setFileType(StorageOptions.FileType.OPENROCKET);
        options.setSaveSimulationData(true);
        options.setExplicitlySet(true);
        try (FileOutputStream stream = new FileOutputStream(path.toFile())) {
            new OpenRocketSaver().save(stream, document, options, new WarningSet(), new ErrorSet());
        }
    }

    private static void writeSummary(Path path, Simulation calm, Simulation crosswind,
                                     OpenRocketDocument document) throws IOException {
        StringBuilder out = new StringBuilder();
        out.append("{\n  \"schema\": \"ksa64.openrocket-summary-v1\",\n");
        out.append("  \"tool\": \"OpenRocket 24.12\",\n");
        out.append("  \"aligned_dry_mass_kg\": ").append(DRY_MASS).append(",\n");
        out.append("  \"rocket_length_m\": ").append(document.getRocket().getLength()).append(",\n");
        out.append("  \"cases\": [\n");
        appendCase(out, "calm", calm);
        out.append(",\n");
        appendCase(out, "steady-crosswind-5mps", crosswind);
        out.append("\n  ]\n}\n");
        Files.writeString(path, out.toString(), StandardCharsets.UTF_8);
    }

    private static void appendCase(StringBuilder out, String name, Simulation simulation) {
        FlightData data = simulation.getSimulatedData();
        FlightDataBranch branch = data.getBranch(0);
        out.append("    {\"name\":\"").append(name).append("\"");
        appendNumber(out, "apogee_m", data.getMaxAltitude());
        appendNumber(out, "max_velocity_mps", data.getMaxVelocity());
        appendNumber(out, "max_acceleration_mps2", data.getMaxAcceleration());
        appendNumber(out, "max_mach", data.getMaxMachNumber());
        appendNumber(out, "time_to_apogee_s", data.getTimeToApogee());
        appendNumber(out, "flight_time_s", data.getFlightTime());
        appendNumber(out, "rail_exit_velocity_mps", data.getLaunchRodVelocity());
        appendNumber(out, "landing_velocity_mps", data.getGroundHitVelocity());
        appendNumber(out, "max_aoa_rad", maxValidatedAoa(branch));
        appendNumber(out, "max_dynamic_pressure_pa", maxDynamicPressure(branch));
        appendNumber(out, "landing_distance_m", lastFinite(branch.get(FlightDataType.TYPE_POSITION_XY)));
        out.append("}");
    }

    private static void appendNumber(StringBuilder out, String name, double value) {
        out.append(",\"").append(name).append("\":").append(Double.toString(value));
    }

    private static double maxValidatedAoa(FlightDataBranch branch) {
        FlightEvent rail = branch.getFirstEvent(FlightEvent.Type.LAUNCHROD);
        FlightEvent apogee = branch.getFirstEvent(FlightEvent.Type.APOGEE);
        List<Double> time = branch.get(FlightDataType.TYPE_TIME);
        List<Double> angle = branch.get(FlightDataType.TYPE_AOA);
        List<Double> density = branch.get(FlightDataType.TYPE_AIR_DENSITY);
        List<Double> velocity = branch.get(FlightDataType.TYPE_VELOCITY_TOTAL);
        double max = 0.0;
        for (int i = 0; i < time.size(); i++) {
            double q = 0.5 * density.get(i) * velocity.get(i) * velocity.get(i);
            if (time.get(i) >= rail.getTime() && time.get(i) <= apogee.getTime() && q >= 50.0
                    && Double.isFinite(angle.get(i))) {
                max = Math.max(max, Math.abs(angle.get(i)));
            }
        }
        return max;
    }
    private static double maxFinite(List<Double> values) {
        double max = Double.NEGATIVE_INFINITY;
        if (values != null) {
            for (double value : values) if (Double.isFinite(value)) max = Math.max(max, Math.abs(value));
        }
        return max;
    }

    private static double lastFinite(List<Double> values) {
        if (values == null) return Double.NaN;
        for (int i = values.size() - 1; i >= 0; i--) if (Double.isFinite(values.get(i))) return values.get(i);
        return Double.NaN;
    }

    private static double maxDynamicPressure(FlightDataBranch branch) {
        List<Double> density = branch.get(FlightDataType.TYPE_AIR_DENSITY);
        List<Double> velocity = branch.get(FlightDataType.TYPE_VELOCITY_TOTAL);
        double max = 0.0;
        if (density == null || velocity == null) return Double.NaN;
        for (int i = 0; i < Math.min(density.size(), velocity.size()); i++) {
            double q = 0.5 * density.get(i) * velocity.get(i) * velocity.get(i);
            if (Double.isFinite(q)) max = Math.max(max, q);
        }
        return max;
    }
}
