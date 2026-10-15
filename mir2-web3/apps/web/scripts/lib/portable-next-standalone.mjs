import { promises as fs } from "node:fs";
import { constants as fsConstants } from "node:fs";
import { realpathSync } from "node:fs";
import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import path from "node:path";

const ROOT_PUBLIC_FILES = new Set([
  "favicon.ico", "favicon.svg", "mir2-asset-worker.js", "original-asset-manifest.generated.json",
]);
const LOCAL_UI_FALLBACKS = new Set([
  "original-ui/Prguse/2092.png", "original-ui/Prguse/2094.png", "original-ui/Prguse/2095.png",
  // Exact static art from Crystal Bag/Storage skin contracts; other remote media stays remote.
  "original-ui/Prguse/586.png",
  "original-ui/Title/0.png",
  "original-ui/Title/743.png",
  "original-ui/Title/744.png",
  "original-ui/Title/745.png",
  "original-ui/Title/746.png",
  "original-ui/Prguse2/360.png",
  "original-ui/Prguse2/361.png",
  "original-ui/Prguse2/362.png",
  "original-ui/Prguse/2443.png",
  "original-ui/Title/483.png",
  "original-ui/Title/484.png",
  "original-ui/Title/485.png",
  "original-ui/Title/113.png",
  "original-ui/Title/114.png",
  "original-ui/Title/115.png",
  "original-ui/Title/196.png",
  "original-ui/Title/197.png",
  "original-ui/Title/737.png",
  "original-ui/Title/169.png",
  "original-ui/Title/168.png",
  "original-ui/Title/738.png",
  "original-ui/Title/198.png",
  "original-ui/Title/739.png",
  "original-ui/Prguse2/366.png",
  "original-ui/Prguse2/367.png",
  "original-ui/Prguse2/368.png",
  "original-ui/Prguse/24.png",
  "original-ui/UI_32bit/471.png",
  "original-ui/UI_32bit/470.png",
  // Exact SHOP_REQUIRED_SKINS additions; shared close skins remain in the union above.
  "original-ui/Prguse/1000.png",
  "original-ui/Prguse2/197.png",
  "original-ui/Prguse2/198.png",
  "original-ui/Prguse2/199.png",
  "original-ui/Prguse2/207.png",
  "original-ui/Prguse2/208.png",
  "original-ui/Prguse2/209.png",
  "original-ui/Prguse2/205.png",
  "original-ui/Title/312.png",
  "original-ui/Title/313.png",
  "original-ui/Title/314.png",
  "original-ui/Prguse/550.png",
  // Canonical Character/Stats/Spells static alternatives, authored hair and four wing frames.
  "original-ui/Prguse/100.png",
  "original-ui/Prguse/101.png",
  "original-ui/Prguse/102.png",
  "original-ui/Prguse/103.png",
  "original-ui/Prguse/104.png",
  "original-ui/Prguse/1656.png",
  "original-ui/Prguse/1657.png",
  "original-ui/Prguse/1658.png",
  "original-ui/Prguse/340.png",
  "original-ui/Prguse/341.png",
  "original-ui/Prguse/396.png",
  "original-ui/Prguse/397.png",
  "original-ui/Prguse/398.png",
  "original-ui/Prguse/399.png",
  "original-ui/Prguse/441.png",
  "original-ui/Prguse/442.png",
  "original-ui/Prguse/443.png",
  "original-ui/Prguse/444.png",
  "original-ui/Prguse/445.png",
  "original-ui/Prguse/446.png",
  "original-ui/Prguse/447.png",
  "original-ui/Prguse/448.png",
  "original-ui/Prguse/449.png",
  "original-ui/Prguse/461.png",
  "original-ui/Prguse/462.png",
  "original-ui/Prguse/463.png",
  "original-ui/Prguse/464.png",
  "original-ui/Prguse/465.png",
  "original-ui/Prguse/466.png",
  "original-ui/Prguse/467.png",
  "original-ui/Prguse/468.png",
  "original-ui/Prguse/469.png",
  "original-ui/Prguse/481.png",
  "original-ui/Prguse/482.png",
  "original-ui/Prguse/483.png",
  "original-ui/Prguse/484.png",
  "original-ui/Prguse/485.png",
  "original-ui/Prguse/486.png",
  "original-ui/Prguse/487.png",
  "original-ui/Prguse/488.png",
  "original-ui/Prguse/489.png",
  "original-ui/Prguse/501.png",
  "original-ui/Prguse/502.png",
  "original-ui/Prguse/503.png",
  "original-ui/Prguse/504.png",
  "original-ui/Prguse/505.png",
  "original-ui/Prguse/506.png",
  "original-ui/Prguse/507.png",
  "original-ui/Prguse/508.png",
  "original-ui/Prguse/509.png",
  "original-ui/Prguse/710.png",
  "original-ui/Prguse2/1202.png",
  "original-ui/Prguse2/1203.png",
  "original-ui/Prguse2/1204.png",
  "original-ui/Prguse2/1205.png",
  "original-ui/Prguse2/1290.png",
  "original-ui/Prguse2/1291.png",
  "original-ui/Prguse2/1292.png",
  "original-ui/Prguse2/1293.png",
  "original-ui/Prguse2/1294.png",
  "original-ui/Prguse2/1295.png",
  "original-ui/Prguse2/1296.png",
  "original-ui/Prguse2/1297.png",
  "original-ui/Prguse2/1298.png",
  "original-ui/Prguse2/1299.png",
  "original-ui/Prguse2/1300.png",
  "original-ui/Prguse2/1301.png",
  "original-ui/Prguse2/1302.png",
  "original-ui/Prguse2/1303.png",
  "original-ui/Prguse2/1304.png",
  "original-ui/Prguse2/1305.png",
  "original-ui/Prguse2/1306.png",
  "original-ui/Prguse2/1307.png",
  "original-ui/Prguse2/1308.png",
  "original-ui/Prguse2/1309.png",
  "original-ui/Prguse2/1310.png",
  "original-ui/Prguse2/1311.png",
  "original-ui/Prguse2/1312.png",
  "original-ui/Prguse2/1313.png",
  "original-ui/Prguse2/1314.png",
  "original-ui/Prguse2/1315.png",
  "original-ui/Prguse2/1316.png",
  "original-ui/Prguse2/1317.png",
  "original-ui/Prguse2/1318.png",
  "original-ui/Prguse2/1319.png",
  "original-ui/Prguse2/1320.png",
  "original-ui/Prguse2/1321.png",
  "original-ui/Prguse2/1322.png",
  "original-ui/Prguse2/1323.png",
  "original-ui/Prguse2/1324.png",
  "original-ui/Title/156.png",
  "original-ui/Title/157.png",
  "original-ui/Title/158.png",
  "original-ui/Title/287.png",
  "original-ui/Title/288.png",
  "original-ui/Title/289.png",
  "original-ui/Title/500.png",
  "original-ui/Title/501.png",
  "original-ui/Title/502.png",
  "original-ui/Title/503.png",
  "original-ui/Title/504.png",
  "original-ui/Title/506.png",
  "original-ui/Title/507.png",
  "original-ui/Title/508.png",
  "original-ui/Title/516.png",
  "original-ui/Title/517.png",
  // Finite public NPC Trade concrete-stack art and canonical initial equipment, Source77.
  "original-ui/Items/0.png",
  "original-ui/Items/1.png",
  "original-ui/Items/100.png",
  "original-ui/Items/1000.png",
  "original-ui/Items/1001.png",
  "original-ui/Items/1002.png",
  "original-ui/Items/1003.png",
  "original-ui/Items/1004.png",
  "original-ui/Items/1005.png",
  "original-ui/Items/1006.png",
  "original-ui/Items/1007.png",
  "original-ui/Items/1008.png",
  "original-ui/Items/1009.png",
  "original-ui/Items/101.png",
  "original-ui/Items/1010.png",
  "original-ui/Items/1011.png",
  "original-ui/Items/1012.png",
  "original-ui/Items/1013.png",
  "original-ui/Items/1014.png",
  "original-ui/Items/1015.png",
  "original-ui/Items/1016.png",
  "original-ui/Items/1017.png",
  "original-ui/Items/1018.png",
  "original-ui/Items/102.png",
  "original-ui/Items/1020.png",
  "original-ui/Items/1021.png",
  "original-ui/Items/1022.png",
  "original-ui/Items/1023.png",
  "original-ui/Items/1024.png",
  "original-ui/Items/1025.png",
  "original-ui/Items/1026.png",
  "original-ui/Items/1027.png",
  "original-ui/Items/1028.png",
  "original-ui/Items/103.png",
  "original-ui/Items/1030.png",
  "original-ui/Items/1031.png",
  "original-ui/Items/1032.png",
  "original-ui/Items/1033.png",
  "original-ui/Items/104.png",
  "original-ui/Items/1040.png",
  "original-ui/Items/1041.png",
  "original-ui/Items/1042.png",
  "original-ui/Items/1043.png",
  "original-ui/Items/105.png",
  "original-ui/Items/106.png",
  "original-ui/Items/1067.png",
  "original-ui/Items/1068.png",
  "original-ui/Items/107.png",
  "original-ui/Items/1078.png",
  "original-ui/Items/108.png",
  "original-ui/Items/109.png",
  "original-ui/Items/1095.png",
  "original-ui/Items/110.png",
  "original-ui/Items/111.png",
  "original-ui/Items/1132.png",
  "original-ui/Items/115.png",
  "original-ui/Items/1150.png",
  "original-ui/Items/1151.png",
  "original-ui/Items/1152.png",
  "original-ui/Items/1153.png",
  "original-ui/Items/1154.png",
  "original-ui/Items/1155.png",
  "original-ui/Items/1156.png",
  "original-ui/Items/1157.png",
  "original-ui/Items/117.png",
  "original-ui/Items/1170.png",
  "original-ui/Items/1173.png",
  "original-ui/Items/1176.png",
  "original-ui/Items/1177.png",
  "original-ui/Items/1178.png",
  "original-ui/Items/1179.png",
  "original-ui/Items/1180.png",
  "original-ui/Items/1182.png",
  "original-ui/Items/119.png",
  "original-ui/Items/121.png",
  "original-ui/Items/122.png",
  "original-ui/Items/13.png",
  "original-ui/Items/130.png",
  "original-ui/Items/131.png",
  "original-ui/Items/132.png",
  "original-ui/Items/1330.png",
  "original-ui/Items/1331.png",
  "original-ui/Items/1332.png",
  "original-ui/Items/14.png",
  "original-ui/Items/140.png",
  "original-ui/Items/1400.png",
  "original-ui/Items/1401.png",
  "original-ui/Items/141.png",
  "original-ui/Items/142.png",
  "original-ui/Items/143.png",
  "original-ui/Items/144.png",
  "original-ui/Items/145.png",
  "original-ui/Items/148.png",
  "original-ui/Items/152.png",
  "original-ui/Items/153.png",
  "original-ui/Items/157.png",
  "original-ui/Items/159.png",
  "original-ui/Items/16.png",
  "original-ui/Items/161.png",
  "original-ui/Items/162.png",
  "original-ui/Items/163.png",
  "original-ui/Items/164.png",
  "original-ui/Items/165.png",
  "original-ui/Items/166.png",
  "original-ui/Items/167.png",
  "original-ui/Items/168.png",
  "original-ui/Items/169.png",
  "original-ui/Items/170.png",
  "original-ui/Items/171.png",
  "original-ui/Items/172.png",
  "original-ui/Items/173.png",
  "original-ui/Items/174.png",
  "original-ui/Items/175.png",
  "original-ui/Items/176.png",
  "original-ui/Items/177.png",
  "original-ui/Items/178.png",
  "original-ui/Items/179.png",
  "original-ui/Items/18.png",
  "original-ui/Items/180.png",
  "original-ui/Items/181.png",
  "original-ui/Items/182.png",
  "original-ui/Items/183.png",
  "original-ui/Items/186.png",
  "original-ui/Items/187.png",
  "original-ui/Items/188.png",
  "original-ui/Items/190.png",
  "original-ui/Items/191.png",
  "original-ui/Items/192.png",
  "original-ui/Items/194.png",
  "original-ui/Items/198.png",
  "original-ui/Items/201.png",
  "original-ui/Items/202.png",
  "original-ui/Items/203.png",
  "original-ui/Items/204.png",
  "original-ui/Items/2040.png",
  "original-ui/Items/2042.png",
  "original-ui/Items/2044.png",
  "original-ui/Items/205.png",
  "original-ui/Items/2060.png",
  "original-ui/Items/2061.png",
  "original-ui/Items/2064.png",
  "original-ui/Items/2065.png",
  "original-ui/Items/2066.png",
  "original-ui/Items/2067.png",
  "original-ui/Items/207.png",
  "original-ui/Items/2070.png",
  "original-ui/Items/208.png",
  "original-ui/Items/209.png",
  "original-ui/Items/210.png",
  "original-ui/Items/211.png",
  "original-ui/Items/2110.png",
  "original-ui/Items/2111.png",
  "original-ui/Items/2112.png",
  "original-ui/Items/2113.png",
  "original-ui/Items/2114.png",
  "original-ui/Items/2116.png",
  "original-ui/Items/2117.png",
  "original-ui/Items/2118.png",
  "original-ui/Items/212.png",
  "original-ui/Items/2120.png",
  "original-ui/Items/2123.png",
  "original-ui/Items/2125.png",
  "original-ui/Items/213.png",
  "original-ui/Items/214.png",
  "original-ui/Items/215.png",
  "original-ui/Items/218.png",
  "original-ui/Items/219.png",
  "original-ui/Items/220.png",
  "original-ui/Items/221.png",
  "original-ui/Items/222.png",
  "original-ui/Items/2240.png",
  "original-ui/Items/2241.png",
  "original-ui/Items/225.png",
  "original-ui/Items/2250.png",
  "original-ui/Items/2251.png",
  "original-ui/Items/2252.png",
  "original-ui/Items/2253.png",
  "original-ui/Items/2254.png",
  "original-ui/Items/2255.png",
  "original-ui/Items/2256.png",
  "original-ui/Items/2258.png",
  "original-ui/Items/2259.png",
  "original-ui/Items/226.png",
  "original-ui/Items/227.png",
  "original-ui/Items/2270.png",
  "original-ui/Items/2271.png",
  "original-ui/Items/2272.png",
  "original-ui/Items/2273.png",
  "original-ui/Items/2274.png",
  "original-ui/Items/2276.png",
  "original-ui/Items/228.png",
  "original-ui/Items/2280.png",
  "original-ui/Items/2281.png",
  "original-ui/Items/2282.png",
  "original-ui/Items/2283.png",
  "original-ui/Items/2284.png",
  "original-ui/Items/2286.png",
  "original-ui/Items/229.png",
  "original-ui/Items/23.png",
  "original-ui/Items/230.png",
  "original-ui/Items/2313.png",
  "original-ui/Items/232.png",
  "original-ui/Items/233.png",
  "original-ui/Items/2330.png",
  "original-ui/Items/2331.png",
  "original-ui/Items/2332.png",
  "original-ui/Items/2333.png",
  "original-ui/Items/2334.png",
  "original-ui/Items/234.png",
  "original-ui/Items/235.png",
  "original-ui/Items/236.png",
  "original-ui/Items/237.png",
  "original-ui/Items/2370.png",
  "original-ui/Items/2380.png",
  "original-ui/Items/239.png",
  "original-ui/Items/2391.png",
  "original-ui/Items/2393.png",
  "original-ui/Items/2394.png",
  "original-ui/Items/2395.png",
  "original-ui/Items/2396.png",
  "original-ui/Items/2398.png",
  "original-ui/Items/2399.png",
  "original-ui/Items/240.png",
  "original-ui/Items/2400.png",
  "original-ui/Items/241.png",
  "original-ui/Items/242.png",
  "original-ui/Items/243.png",
  "original-ui/Items/244.png",
  "original-ui/Items/245.png",
  "original-ui/Items/246.png",
  "original-ui/Items/247.png",
  "original-ui/Items/248.png",
  "original-ui/Items/249.png",
  "original-ui/Items/252.png",
  "original-ui/Items/253.png",
  "original-ui/Items/254.png",
  "original-ui/Items/255.png",
  "original-ui/Items/256.png",
  "original-ui/Items/257.png",
  "original-ui/Items/26.png",
  "original-ui/Items/260.png",
  "original-ui/Items/261.png",
  "original-ui/Items/264.png",
  "original-ui/Items/271.png",
  "original-ui/Items/273.png",
  "original-ui/Items/2760.png",
  "original-ui/Items/2761.png",
  "original-ui/Items/2762.png",
  "original-ui/Items/2763.png",
  "original-ui/Items/2764.png",
  "original-ui/Items/277.png",
  "original-ui/Items/2770.png",
  "original-ui/Items/28.png",
  "original-ui/Items/280.png",
  "original-ui/Items/281.png",
  "original-ui/Items/282.png",
  "original-ui/Items/283.png",
  "original-ui/Items/284.png",
  "original-ui/Items/285.png",
  "original-ui/Items/286.png",
  "original-ui/Items/287.png",
  "original-ui/Items/2880.png",
  "original-ui/Items/2882.png",
  "original-ui/Items/29.png",
  "original-ui/Items/2903.png",
  "original-ui/Items/2930.png",
  "original-ui/Items/2931.png",
  "original-ui/Items/296.png",
  "original-ui/Items/2960.png",
  "original-ui/Items/2961.png",
  "original-ui/Items/297.png",
  "original-ui/Items/298.png",
  "original-ui/Items/30.png",
  "original-ui/Items/3027.png",
  "original-ui/Items/31.png",
  "original-ui/Items/312.png",
  "original-ui/Items/3140.png",
  "original-ui/Items/3141.png",
  "original-ui/Items/3142.png",
  "original-ui/Items/3143.png",
  "original-ui/Items/3144.png",
  "original-ui/Items/3145.png",
  "original-ui/Items/3146.png",
  "original-ui/Items/3147.png",
  "original-ui/Items/3148.png",
  "original-ui/Items/32.png",
  "original-ui/Items/320.png",
  "original-ui/Items/3200.png",
  "original-ui/Items/3201.png",
  "original-ui/Items/3202.png",
  "original-ui/Items/3203.png",
  "original-ui/Items/3204.png",
  "original-ui/Items/3205.png",
  "original-ui/Items/3206.png",
  "original-ui/Items/3207.png",
  "original-ui/Items/3208.png",
  "original-ui/Items/3209.png",
  "original-ui/Items/3210.png",
  "original-ui/Items/3211.png",
  "original-ui/Items/3212.png",
  "original-ui/Items/3213.png",
  "original-ui/Items/3214.png",
  "original-ui/Items/3215.png",
  "original-ui/Items/3216.png",
  "original-ui/Items/3217.png",
  "original-ui/Items/3218.png",
  "original-ui/Items/3219.png",
  "original-ui/Items/3220.png",
  "original-ui/Items/3221.png",
  "original-ui/Items/3222.png",
  "original-ui/Items/3223.png",
  "original-ui/Items/3224.png",
  "original-ui/Items/3225.png",
  "original-ui/Items/3226.png",
  "original-ui/Items/3227.png",
  "original-ui/Items/328.png",
  "original-ui/Items/3290.png",
  "original-ui/Items/33.png",
  "original-ui/Items/3330.png",
  "original-ui/Items/3331.png",
  "original-ui/Items/3332.png",
  "original-ui/Items/3333.png",
  "original-ui/Items/336.png",
  "original-ui/Items/3367.png",
  "original-ui/Items/3368.png",
  "original-ui/Items/3390.png",
  "original-ui/Items/3391.png",
  "original-ui/Items/3393.png",
  "original-ui/Items/3394.png",
  "original-ui/Items/3397.png",
  "original-ui/Items/3398.png",
  "original-ui/Items/3399.png",
  "original-ui/Items/34.png",
  "original-ui/Items/3400.png",
  "original-ui/Items/3401.png",
  "original-ui/Items/3402.png",
  "original-ui/Items/3403.png",
  "original-ui/Items/3405.png",
  "original-ui/Items/3406.png",
  "original-ui/Items/3407.png",
  "original-ui/Items/3410.png",
  "original-ui/Items/3420.png",
  "original-ui/Items/3421.png",
  "original-ui/Items/3422.png",
  "original-ui/Items/3423.png",
  "original-ui/Items/3424.png",
  "original-ui/Items/3430.png",
  "original-ui/Items/3431.png",
  "original-ui/Items/3432.png",
  "original-ui/Items/3433.png",
  "original-ui/Items/3434.png",
  "original-ui/Items/344.png",
  "original-ui/Items/3470.png",
  "original-ui/Items/3475.png",
  "original-ui/Items/3480.png",
  "original-ui/Items/3489.png",
  "original-ui/Items/3494.png",
  "original-ui/Items/3499.png",
  "original-ui/Items/35.png",
  "original-ui/Items/3576.png",
  "original-ui/Items/36.png",
  "original-ui/Items/360.png",
  "original-ui/Items/3640.png",
  "original-ui/Items/3641.png",
  "original-ui/Items/3642.png",
  "original-ui/Items/3643.png",
  "original-ui/Items/3644.png",
  "original-ui/Items/3660.png",
  "original-ui/Items/3661.png",
  "original-ui/Items/3662.png",
  "original-ui/Items/3670.png",
  "original-ui/Items/3671.png",
  "original-ui/Items/3672.png",
  "original-ui/Items/3673.png",
  "original-ui/Items/3674.png",
  "original-ui/Items/3675.png",
  "original-ui/Items/368.png",
  "original-ui/Items/37.png",
  "original-ui/Items/3756.png",
  "original-ui/Items/3757.png",
  "original-ui/Items/3759.png",
  "original-ui/Items/3760.png",
  "original-ui/Items/3761.png",
  "original-ui/Items/3775.png",
  "original-ui/Items/3776.png",
  "original-ui/Items/3783.png",
  "original-ui/Items/3784.png",
  "original-ui/Items/3785.png",
  "original-ui/Items/3786.png",
  "original-ui/Items/38.png",
  "original-ui/Items/39.png",
  "original-ui/Items/391.png",
  "original-ui/Items/392.png",
  "original-ui/Items/394.png",
  "original-ui/Items/396.png",
  "original-ui/Items/398.png",
  "original-ui/Items/40.png",
  "original-ui/Items/400.png",
  "original-ui/Items/402.png",
  "original-ui/Items/404.png",
  "original-ui/Items/406.png",
  "original-ui/Items/408.png",
  "original-ui/Items/41.png",
  "original-ui/Items/412.png",
  "original-ui/Items/42.png",
  "original-ui/Items/428.png",
  "original-ui/Items/429.png",
  "original-ui/Items/43.png",
  "original-ui/Items/430.png",
  "original-ui/Items/432.png",
  "original-ui/Items/433.png",
  "original-ui/Items/434.png",
  "original-ui/Items/44.png",
  "original-ui/Items/444.png",
  "original-ui/Items/446.png",
  "original-ui/Items/447.png",
  "original-ui/Items/448.png",
  "original-ui/Items/45.png",
  "original-ui/Items/452.png",
  "original-ui/Items/46.png",
  "original-ui/Items/47.png",
  "original-ui/Items/48.png",
  "original-ui/Items/481.png",
  "original-ui/Items/482.png",
  "original-ui/Items/483.png",
  "original-ui/Items/484.png",
  "original-ui/Items/49.png",
  "original-ui/Items/490.png",
  "original-ui/Items/491.png",
  "original-ui/Items/50.png",
  "original-ui/Items/502.png",
  "original-ui/Items/503.png",
  "original-ui/Items/505.png",
  "original-ui/Items/51.png",
  "original-ui/Items/510.png",
  "original-ui/Items/511.png",
  "original-ui/Items/512.png",
  "original-ui/Items/513.png",
  "original-ui/Items/514.png",
  "original-ui/Items/515.png",
  "original-ui/Items/516.png",
  "original-ui/Items/517.png",
  "original-ui/Items/518.png",
  "original-ui/Items/52.png",
  "original-ui/Items/520.png",
  "original-ui/Items/521.png",
  "original-ui/Items/522.png",
  "original-ui/Items/523.png",
  "original-ui/Items/524.png",
  "original-ui/Items/525.png",
  "original-ui/Items/526.png",
  "original-ui/Items/527.png",
  "original-ui/Items/528.png",
  "original-ui/Items/529.png",
  "original-ui/Items/53.png",
  "original-ui/Items/530.png",
  "original-ui/Items/531.png",
  "original-ui/Items/532.png",
  "original-ui/Items/533.png",
  "original-ui/Items/534.png",
  "original-ui/Items/54.png",
  "original-ui/Items/540.png",
  "original-ui/Items/541.png",
  "original-ui/Items/542.png",
  "original-ui/Items/543.png",
  "original-ui/Items/544.png",
  "original-ui/Items/545.png",
  "original-ui/Items/546.png",
  "original-ui/Items/547.png",
  "original-ui/Items/548.png",
  "original-ui/Items/55.png",
  "original-ui/Items/550.png",
  "original-ui/Items/551.png",
  "original-ui/Items/552.png",
  "original-ui/Items/553.png",
  "original-ui/Items/554.png",
  "original-ui/Items/555.png",
  "original-ui/Items/557.png",
  "original-ui/Items/558.png",
  "original-ui/Items/56.png",
  "original-ui/Items/560.png",
  "original-ui/Items/561.png",
  "original-ui/Items/562.png",
  "original-ui/Items/563.png",
  "original-ui/Items/564.png",
  "original-ui/Items/565.png",
  "original-ui/Items/568.png",
  "original-ui/Items/57.png",
  "original-ui/Items/575.png",
  "original-ui/Items/576.png",
  "original-ui/Items/577.png",
  "original-ui/Items/578.png",
  "original-ui/Items/579.png",
  "original-ui/Items/58.png",
  "original-ui/Items/580.png",
  "original-ui/Items/581.png",
  "original-ui/Items/582.png",
  "original-ui/Items/583.png",
  "original-ui/Items/584.png",
  "original-ui/Items/585.png",
  "original-ui/Items/586.png",
  "original-ui/Items/587.png",
  "original-ui/Items/588.png",
  "original-ui/Items/589.png",
  "original-ui/Items/59.png",
  "original-ui/Items/590.png",
  "original-ui/Items/591.png",
  "original-ui/Items/592.png",
  "original-ui/Items/595.png",
  "original-ui/Items/596.png",
  "original-ui/Items/597.png",
  "original-ui/Items/6.png",
  "original-ui/Items/60.png",
  "original-ui/Items/600.png",
  "original-ui/Items/601.png",
  "original-ui/Items/602.png",
  "original-ui/Items/605.png",
  "original-ui/Items/606.png",
  "original-ui/Items/607.png",
  "original-ui/Items/61.png",
  "original-ui/Items/610.png",
  "original-ui/Items/611.png",
  "original-ui/Items/612.png",
  "original-ui/Items/613.png",
  "original-ui/Items/614.png",
  "original-ui/Items/615.png",
  "original-ui/Items/616.png",
  "original-ui/Items/617.png",
  "original-ui/Items/618.png",
  "original-ui/Items/619.png",
  "original-ui/Items/62.png",
  "original-ui/Items/620.png",
  "original-ui/Items/621.png",
  "original-ui/Items/622.png",
  "original-ui/Items/623.png",
  "original-ui/Items/624.png",
  "original-ui/Items/63.png",
  "original-ui/Items/64.png",
  "original-ui/Items/66.png",
  "original-ui/Items/67.png",
  "original-ui/Items/670.png",
  "original-ui/Items/671.png",
  "original-ui/Items/672.png",
  "original-ui/Items/673.png",
  "original-ui/Items/674.png",
  "original-ui/Items/675.png",
  "original-ui/Items/676.png",
  "original-ui/Items/677.png",
  "original-ui/Items/678.png",
  "original-ui/Items/679.png",
  "original-ui/Items/68.png",
  "original-ui/Items/680.png",
  "original-ui/Items/681.png",
  "original-ui/Items/682.png",
  "original-ui/Items/683.png",
  "original-ui/Items/684.png",
  "original-ui/Items/69.png",
  "original-ui/Items/690.png",
  "original-ui/Items/70.png",
  "original-ui/Items/700.png",
  "original-ui/Items/701.png",
  "original-ui/Items/702.png",
  "original-ui/Items/703.png",
  "original-ui/Items/704.png",
  "original-ui/Items/705.png",
  "original-ui/Items/71.png",
  "original-ui/Items/710.png",
  "original-ui/Items/711.png",
  "original-ui/Items/712.png",
  "original-ui/Items/713.png",
  "original-ui/Items/714.png",
  "original-ui/Items/715.png",
  "original-ui/Items/72.png",
  "original-ui/Items/720.png",
  "original-ui/Items/721.png",
  "original-ui/Items/722.png",
  "original-ui/Items/723.png",
  "original-ui/Items/724.png",
  "original-ui/Items/725.png",
  "original-ui/Items/726.png",
  "original-ui/Items/73.png",
  "original-ui/Items/730.png",
  "original-ui/Items/731.png",
  "original-ui/Items/732.png",
  "original-ui/Items/733.png",
  "original-ui/Items/734.png",
  "original-ui/Items/735.png",
  "original-ui/Items/736.png",
  "original-ui/Items/74.png",
  "original-ui/Items/740.png",
  "original-ui/Items/741.png",
  "original-ui/Items/742.png",
  "original-ui/Items/743.png",
  "original-ui/Items/744.png",
  "original-ui/Items/745.png",
  "original-ui/Items/746.png",
  "original-ui/Items/75.png",
  "original-ui/Items/750.png",
  "original-ui/Items/751.png",
  "original-ui/Items/752.png",
  "original-ui/Items/753.png",
  "original-ui/Items/754.png",
  "original-ui/Items/755.png",
  "original-ui/Items/756.png",
  "original-ui/Items/757.png",
  "original-ui/Items/758.png",
  "original-ui/Items/759.png",
  "original-ui/Items/76.png",
  "original-ui/Items/760.png",
  "original-ui/Items/761.png",
  "original-ui/Items/762.png",
  "original-ui/Items/763.png",
  "original-ui/Items/764.png",
  "original-ui/Items/765.png",
  "original-ui/Items/766.png",
  "original-ui/Items/767.png",
  "original-ui/Items/768.png",
  "original-ui/Items/769.png",
  "original-ui/Items/77.png",
  "original-ui/Items/770.png",
  "original-ui/Items/771.png",
  "original-ui/Items/772.png",
  "original-ui/Items/773.png",
  "original-ui/Items/774.png",
  "original-ui/Items/775.png",
  "original-ui/Items/776.png",
  "original-ui/Items/777.png",
  "original-ui/Items/778.png",
  "original-ui/Items/779.png",
  "original-ui/Items/780.png",
  "original-ui/Items/781.png",
  "original-ui/Items/782.png",
  "original-ui/Items/783.png",
  "original-ui/Items/784.png",
  "original-ui/Items/785.png",
  "original-ui/Items/786.png",
  "original-ui/Items/787.png",
  "original-ui/Items/788.png",
  "original-ui/Items/789.png",
  "original-ui/Items/80.png",
  "original-ui/Items/802.png",
  "original-ui/Items/803.png",
  "original-ui/Items/804.png",
  "original-ui/Items/805.png",
  "original-ui/Items/806.png",
  "original-ui/Items/807.png",
  "original-ui/Items/808.png",
  "original-ui/Items/809.png",
  "original-ui/Items/81.png",
  "original-ui/Items/810.png",
  "original-ui/Items/811.png",
  "original-ui/Items/812.png",
  "original-ui/Items/813.png",
  "original-ui/Items/814.png",
  "original-ui/Items/82.png",
  "original-ui/Items/820.png",
  "original-ui/Items/821.png",
  "original-ui/Items/822.png",
  "original-ui/Items/823.png",
  "original-ui/Items/824.png",
  "original-ui/Items/825.png",
  "original-ui/Items/826.png",
  "original-ui/Items/827.png",
  "original-ui/Items/828.png",
  "original-ui/Items/83.png",
  "original-ui/Items/837.png",
  "original-ui/Items/84.png",
  "original-ui/Items/849.png",
  "original-ui/Items/850.png",
  "original-ui/Items/851.png",
  "original-ui/Items/852.png",
  "original-ui/Items/853.png",
  "original-ui/Items/854.png",
  "original-ui/Items/855.png",
  "original-ui/Items/857.png",
  "original-ui/Items/859.png",
  "original-ui/Items/86.png",
  "original-ui/Items/87.png",
  "original-ui/Items/870.png",
  "original-ui/Items/871.png",
  "original-ui/Items/872.png",
  "original-ui/Items/874.png",
  "original-ui/Items/878.png",
  "original-ui/Items/893.png",
  "original-ui/Items/894.png",
  "original-ui/Items/895.png",
  "original-ui/Items/896.png",
  "original-ui/Items/897.png",
  "original-ui/Items/898.png",
  "original-ui/Items/899.png",
  "original-ui/Items/900.png",
  "original-ui/Items/901.png",
  "original-ui/Items/902.png",
  "original-ui/Items/903.png",
  "original-ui/Items/904.png",
  "original-ui/Items/915.png",
  "original-ui/Items/934.png",
  "original-ui/Items/941.png",
  "original-ui/Items/960.png",
  "original-ui/Items/961.png",
  "original-ui/Items/962.png",
  "original-ui/Items/970.png",
  "original-ui/Items/971.png",
  "original-ui/Items/972.png",
  "original-ui/StateItem/2259.png",
  "original-ui/StateItem/30.png",
  "original-ui/StateItem/3390.png",
  "original-ui/StateItem/60.png",
  "original-ui/StateItem/80.png",
]);
const LOCAL_DEBUG_FILES = new Set([
  "debug/map-samples/smtile-72.png", "debug/map-samples/smtile-80.png",
]);
const WHOLE_PUBLIC_ROOTS = new Set([
  "bevy-entity-atlases", "bootstrap", "original-effects", "pwa",
]);
const MAP_PUBLIC_ROOTS = new Set(["map-atlas", "original-map-blend"]);
const REMOTE_UI_MEDIA_ROOTS = new Set([
  "AArmour", "AHair", "ARArmour", "ARHair", "ARWeapon", "AWeapon", "CArmour", "CHair",
  "ChrSel", "CWeapon", "Cursors", "Items", "MMap", "MapLinkIcon", "Monster", "NPC",
  "Prguse", "Prguse2", "Sound", "Title",
  // These measured non-actor libraries use the existing original-ui miss proxy.
  "Help", "DNItems", "StateItem", "MagIcon", "MagIcon2", "BuffIcon", "GuildSkill",
  // These measured actor/door roots use that proxy as well.
  "Mount", "Pet", "Gate",
]);
const REQUIRED_MODULES = ["next", "next/dist/server/lib/start-server", "react", "react-dom", "sharp"];

function segmentsOf(relativePath) {
  if (typeof relativePath !== "string" || path.win32.isAbsolute(relativePath) || path.posix.isAbsolute(relativePath) ||
      relativePath.includes("\0") || relativePath.includes(":")) {
    throw new Error(`Unsafe portable path: ${relativePath}`);
  }
  if (!relativePath) return [];
  const segments = relativePath.replaceAll("\\", "/").split("/");
  if (segments.some((segment) => !segment || segment === "." || segment === "..")) {
    throw new Error(`Unsafe portable path: ${relativePath}`);
  }
  return segments;
}

function inside(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === "" || (relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
}

function requireInside(root, candidate) {
  if (!path.isAbsolute(root) || !path.isAbsolute(candidate) || !inside(root, candidate)) {
    throw new Error(`Portable path escapes its root: ${candidate}`);
  }
}

function isSensitivePath(candidate) {
  return String(candidate).split(/[\\/]/).some((segment) => {
    const name = segment.toLowerCase();
    return name === ".env" || name.startsWith(".env.") || /\.(?:pem|key|p12|pfx)$/.test(name);
  });
}

export function selectThinPublicEntry(relativePath, isDirectory, { runtimeVersion, runtimeFiles, mapAtlasFiles = null, localMapFiles = new Set(), coreVersion = null, coreFiles = new Set() }) {
  const segments = segmentsOf(relativePath);
  if (segments.length === 0) return isDirectory;
  const normalized = segments.join("/");
  if (isSensitivePath(normalized)) return false;
  const [root, second] = segments;
  if (root === "client-core") {
    return isDirectory
      ? segments.length === 1 || ([...coreFiles].some((file) => file.startsWith(`${normalized}/`)))
      : coreFiles.has(normalized);
  }
  if (root === "bevy-runtime-releases") {
    if (isDirectory) {
      return segments.length === 1 || (second === runtimeVersion && [...runtimeFiles].some((file) =>
        file.startsWith(`${normalized}/`)));
    }
    return runtimeFiles.has(normalized);
  }
  if (root === "generated") {
    if (isDirectory && segments.length === 1) return true;
    if (second === "map-atlas" && mapAtlasFiles) {
      return isDirectory
        ? [...mapAtlasFiles].some((file) => file.startsWith(`${normalized}/`))
        : mapAtlasFiles.has(normalized);
    }
    return MAP_PUBLIC_ROOTS.has(second) && (isDirectory || segments.length > 2);
  }
  if (root === "original-map") {
    return isDirectory
      ? [...localMapFiles].some((file) => file.startsWith(`${normalized}/`))
      : localMapFiles.has(normalized);
  }
  if (root === "original-ui") {
    if (isDirectory) return true;
    const name = segments.at(-1);
    const supportFile = /^(?:LICENSE(?:\..*)?|COPYING(?:\..*)?|NOTICE(?:\..*)?)$/i.test(name) ||
      /\.(?:otf|ttf|woff2?)$/i.test(name);
    return !REMOTE_UI_MEDIA_ROOTS.has(second) || LOCAL_UI_FALLBACKS.has(normalized) ||
      normalized.endsWith(".json") || supportFile;
  }
  if (root === "debug") {
    return isDirectory ? segments.length <= 2 : LOCAL_DEBUG_FILES.has(normalized);
  }
  if (WHOLE_PUBLIC_ROOTS.has(root)) return isDirectory || segments.length > 1;
  return !isDirectory && segments.length === 1 && ROOT_PUBLIC_FILES.has(root);
}

export function classifyPortableStandalonePath(relativePath, appRelativePath, isDirectory, publicSelection) {
  const segments = segmentsOf(relativePath);
  const app = segmentsOf(appRelativePath);
  if (app.every((segment, index) => segments[index] === segment) && segments.length > app.length) {
    const tail = segments.slice(app.length);
    if (tail[0] === "node_modules") return { copy: false, reason: "external dependency junction" };
    if (tail[0] === ".next" && tail[1] === "cache" && tail[2] === "mir2-scene-blueprints") {
      return { copy: false, reason: "scene cache seed" };
    }
    if (tail.at(-1).endsWith(".nft.json")) return { copy: false, reason: "copied NFT metadata" };
    if (tail[0] === "public") {
      const publicRelative = tail.slice(1).join("/");
      if (!publicSelection(publicRelative, isDirectory)) return { copy: false, reason: "thin public selection" };
    }
  }
  if (segments.some((segment) => isSensitivePath(segment))) {
    return { copy: false, reason: "private build input" };
  }
  return { copy: true, reason: null };
}

export async function copyPortableStandalone({ sourceRoot, destinationRoot, appRelativePath, publicSelection }) {
  if (!path.isAbsolute(sourceRoot) || !path.isAbsolute(destinationRoot)) throw new Error("Portable copy roots must be absolute");
  if (inside(sourceRoot, destinationRoot) || inside(destinationRoot, sourceRoot)) {
    throw new Error("Portable copy roots must not overlap");
  }
  if ((await fs.lstat(sourceRoot)).isSymbolicLink()) throw new Error("Portable standalone source root is a link");
  if ((await fs.lstat(sourceRoot)).isDirectory() !== true) throw new Error("Portable standalone source root is not a directory");
  await assertAbsent(destinationRoot);
  await fs.mkdir(destinationRoot);
  const result = { files: 0, bytes: 0, sourceBytes: 0, skipped: {}, jsonCompactions: [] };
  async function visit(sourceDirectory, destinationDirectory, relativeDirectory) {
    for (const entry of await fs.readdir(sourceDirectory, { withFileTypes: true })) {
      const relative = relativeDirectory ? `${relativeDirectory}/${entry.name}` : entry.name;
      const source = path.resolve(sourceRoot, ...segmentsOf(relative));
      const destination = path.resolve(destinationRoot, ...segmentsOf(relative));
      requireInside(sourceRoot, source);
      requireInside(destinationRoot, destination);
      const stat = await fs.lstat(source);
      const decision = classifyPortableStandalonePath(relative, appRelativePath, stat.isDirectory(), publicSelection);
      if (!decision.copy) {
        result.skipped[decision.reason] = (result.skipped[decision.reason] ?? 0) + 1;
        continue;
      }
      if (stat.isSymbolicLink()) throw new Error(`Unclassified standalone link: ${relative}`);
      if (stat.isDirectory()) {
        await fs.mkdir(destination);
        await visit(source, destination, relative);
      } else if (stat.isFile()) {
        const compact = relative.slice(appRelativePath.length + 1).replaceAll("\\", "/");
        const outcome = await copySelectedFile(source, destination,
          compact.startsWith("public/") ? compact.slice("public/".length) : null);
        if (outcome.compaction) result.jsonCompactions.push(outcome.compaction);
        result.files += 1;
        result.bytes += outcome.outputBytes;
        result.sourceBytes += stat.size;
      } else {
        throw new Error(`Unsupported standalone entry: ${relative}`);
      }
    }
  }
  await visit(sourceRoot, destinationRoot, "");
  return result;
}

export async function copySelectedPublic({ sourceRoot, destinationRoot, selection }) {
  if (!path.isAbsolute(sourceRoot) || !path.isAbsolute(destinationRoot)) throw new Error("Public copy roots must be absolute");
  if (inside(sourceRoot, destinationRoot) || inside(destinationRoot, sourceRoot)) {
    throw new Error("Public copy roots must not overlap");
  }
  if ((await fs.lstat(sourceRoot)).isSymbolicLink()) throw new Error("Source public root is a link");
  await fs.mkdir(destinationRoot, { recursive: true });
  let files = 0;
  let collisions = 0;
  const jsonCompactions = [];
  async function visit(sourceDirectory, destinationDirectory, relativeDirectory) {
    for (const entry of await fs.readdir(sourceDirectory, { withFileTypes: true })) {
      const relative = relativeDirectory ? `${relativeDirectory}/${entry.name}` : entry.name;
      const source = path.resolve(sourceRoot, ...segmentsOf(relative));
      const destination = path.resolve(destinationRoot, ...segmentsOf(relative));
      requireInside(sourceRoot, source);
      requireInside(destinationRoot, destination);
      const stat = await fs.lstat(source);
      if (!selection(relative, stat.isDirectory())) continue;
      if (stat.isSymbolicLink()) throw new Error(`Selected public asset is a link: ${relative}`);
      if (stat.isDirectory()) {
        await fs.mkdir(destination, { recursive: true });
        await visit(source, destination, relative);
      } else if (stat.isFile()) {
        await fs.mkdir(path.dirname(destination), { recursive: true });
        const outcome = await copySelectedFile(source, destination, relative);
        if (outcome.collision) collisions += 1;
        if (outcome.compaction) jsonCompactions.push(outcome.compaction);
        files += 1;
      } else {
        throw new Error(`Unsupported public entry: ${relative}`);
      }
    }
  }
  await visit(sourceRoot, destinationRoot, "");
  return { files, collisions, jsonCompactions };
}

/** Remove only JSON grammar whitespace outside strings, leaving every token byte intact. */
export function compactJsonWhitespace(source) {
  const input = Buffer.isBuffer(source) ? source : Buffer.from(source);
  const output = Buffer.allocUnsafe(input.length);
  let length = 0;
  let inString = false;
  let escaped = false;
  for (const byte of input) {
    if (inString) {
      output[length++] = byte;
      if (escaped) escaped = false;
      else if (byte === 0x5c) escaped = true;
      else if (byte === 0x22) inString = false;
    } else if (byte === 0x22) {
      inString = true;
      output[length++] = byte;
    } else if (byte !== 0x20 && byte !== 0x09 && byte !== 0x0a && byte !== 0x0d) {
      output[length++] = byte;
    }
  }
  if (inString) throw new Error("Unterminated JSON string while compacting public metadata");
  JSON.parse(input.toString("utf8"));
  JSON.parse(output.subarray(0, length).toString("utf8"));
  return output.subarray(0, length);
}

async function copySelectedFile(source, destination, publicRelative) {
  const compact = publicRelative === "original-asset-manifest.generated.json" ||
    publicRelative === "bevy-entity-atlases/manifest.json" ||
    (publicRelative?.startsWith("original-ui/") && publicRelative.endsWith(".json"));
  const sourceBytes = compact ? await fs.readFile(source) : null;
  const outputBytes = compact ? compactJsonWhitespace(sourceBytes) : null;
  const compaction = compact ? {
    path: publicRelative,
    sourceBytes: sourceBytes.length,
    outputBytes: outputBytes.length,
    sourceSha256: createHash("sha256").update(sourceBytes).digest("hex"),
    outputSha256: createHash("sha256").update(outputBytes).digest("hex"),
  } : null;
  let collision = false;
  try {
    const existing = await fs.lstat(destination);
    if (!existing.isFile() || existing.isSymbolicLink()) throw new Error(`Selected destination is not a regular file: ${destination}`);
    const actual = await fs.readFile(destination);
    const expected = outputBytes ?? await fs.readFile(source);
    if (!actual.equals(expected)) throw new Error(`Standalone/source public collision differs: ${publicRelative ?? destination}`);
    collision = true;
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    if (outputBytes) await fs.writeFile(destination, outputBytes, { flag: "wx" });
    else await fs.copyFile(source, destination, fsConstants.COPYFILE_EXCL);
  }
  return { collision, compaction, outputBytes: outputBytes?.length ?? (await fs.stat(source)).size };
}

export async function selectMapAtlasClosure(publicRoot, { pinnedManifestPath = "", pinnedContentHash = "", pinnedEnabled = false } = {}) {
  if (!path.isAbsolute(publicRoot) || (await fs.lstat(publicRoot)).isSymbolicLink()) {
    throw new Error("Map atlas public root must be an absolute real directory");
  }
  const files = new Set();
  const manifests = [];
  if (pinnedEnabled || pinnedManifestPath || pinnedContentHash) {
    const match = /^\/generated\/map-atlas\/manifest\.([a-f0-9]{64})\.json$/i.exec(pinnedManifestPath);
    if (!match || !pinnedContentHash || match[1].toLowerCase() !== pinnedContentHash.toLowerCase()) {
      throw new Error("Compiled pinned map-atlas path/hash is incomplete or inconsistent");
    }
    manifests.push(pinnedManifestPath.slice(1));
  }
  manifests.unshift("generated/map-atlas/manifest.json");
  for (const relative of manifests) {
    const raw = await readRegularPublicFile(publicRoot, relative);
    if (relative !== "generated/map-atlas/manifest.json") {
      const digest = createHash("sha256").update(raw).digest("hex");
      if (digest !== pinnedContentHash.toLowerCase()) throw new Error("Pinned map-atlas bytes do not match compiled hash");
    }
    let manifest;
    try { manifest = JSON.parse(raw.toString("utf8")); }
    catch (error) { throw new Error(`Malformed map-atlas manifest: ${relative}`, { cause: error }); }
    if (manifest?.schemaVersion !== 2 || manifest.kind !== "mir2-map-atlas-manifest" ||
        !Array.isArray(manifest.pages) || manifest.pages.length === 0) {
      throw new Error(`Invalid map-atlas manifest shape: ${relative}`);
    }
    files.add(relative);
    const pagePaths = new Set();
    const pageKeys = new Set();
    for (const page of manifest.pages) {
      if (!page || typeof page.u !== "string" ||
          !/^\/generated\/map-atlas\/[A-Za-z0-9._/-]+\.png$/.test(page.u) ||
          typeof page.l !== "string" || !page.l || !Number.isInteger(page.p) || page.p < 0) {
        throw new Error(`Invalid map-atlas page in ${relative}`);
      }
      const pageRelative = page.u.slice(1);
      segmentsOf(pageRelative);
      const pageKey = `${page.l}#${page.p}`;
      if (pagePaths.has(pageRelative) || pageKeys.has(pageKey)) {
        throw new Error(`Duplicate map-atlas page in ${relative}: ${pageRelative}`);
      }
      pagePaths.add(pageRelative);
      pageKeys.add(pageKey);
      await readRegularPublicFile(publicRoot, pageRelative, false);
      files.add(pageRelative);
    }
  }
  return { files, manifests, pages: files.size - manifests.length };
}


// An explicit finite scene diagnostic is a local delivery request, not a
// replacement for the complete original-asset inventory or remote release.
export async function selectLocalMapClosure(publicRoot, { sourcePath = "", sourceSha256 = "" } = {}) {
  const empty = { files: new Set(), frames: [], bytes: 0, sourceSha256: null };
  if (sourcePath === "" && sourceSha256 === "") return empty;
  if (typeof sourcePath !== "string" || !path.isAbsolute(sourcePath) ||
      typeof sourceSha256 !== "string" || !/^[a-f0-9]{64}$/i.test(sourceSha256)) {
    throw new Error("Local map closure requires an absolute input path and SHA256 pin");
  }
  if (!path.isAbsolute(publicRoot) || (await fs.lstat(publicRoot)).isSymbolicLink() ||
      !(await fs.lstat(publicRoot)).isDirectory()) {
    throw new Error("Local map public root must be an absolute real directory");
  }
  const input = await readRegularPublicFile(path.dirname(sourcePath), path.basename(sourcePath));
  const inputHash = createHash("sha256").update(input).digest("hex");
  if (inputHash !== sourceSha256.toLowerCase()) throw new Error("Local map closure input bytes do not match pin");
  const request = JSON.parse(input.toString("utf8"));
  if (!Array.isArray(request?.missingAssets) || request.missingAssets.length === 0 ||
      request.missingAssets.length > 116) {
    throw new Error("Local map closure requires 1 to 116 explicit frames");
  }
  const manifest = JSON.parse((await readRegularPublicFile(publicRoot,
    "original-asset-manifest.generated.json")).toString("utf8"));
  if (manifest?.schemaVersion !== 1 || manifest.kind !== "mir2-original-asset-manifest" ||
      !manifest.assets || typeof manifest.assets !== "object" || Array.isArray(manifest.assets)) {
    throw new Error("Invalid complete original-asset manifest for local map closure");
  }
  const files = new Set(), frames = [];
  let bytes = 0;
  for (const frame of request.missingAssets) {
    if (!frame || typeof frame.libraryKey !== "string" ||
        !/^(?:[A-Za-z0-9_-]+\/)+[A-Za-z0-9_-]+$/.test(frame.libraryKey) ||
        !Number.isSafeInteger(frame.frameIndex) || frame.frameIndex < 0 ||
        frame.publicPath !== `/original-map/${frame.libraryKey}/${frame.frameIndex}.png`) {
      throw new Error("Unsafe or inconsistent local map frame path");
    }
    const relative = frame.publicPath.slice(1);
    segmentsOf(relative);
    if (files.has(relative)) throw new Error("Duplicate local map frame");
    const entry = Object.hasOwn(manifest.assets, frame.publicPath) ? manifest.assets[frame.publicPath] : null;
    if (!entry || entry.source !== "original-map" || !Number.isSafeInteger(entry.size) || entry.size <= 0 ||
        typeof entry.sha256 !== "string" || !/^[a-f0-9]{64}$/i.test(entry.sha256)) {
      throw new Error("Local map frame is missing a valid complete-manifest record");
    }
    const png = await readRegularPublicFile(publicRoot, relative);
    if (png.length !== entry.size || createHash("sha256").update(png).digest("hex") !== entry.sha256.toLowerCase()) {
      throw new Error("Local map frame bytes do not match complete-manifest size/SHA256");
    }
    files.add(relative);
    frames.push({ path: relative, bytes: png.length, sha256: entry.sha256.toLowerCase() });
    bytes += png.length;
  }
  frames.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  return { files, frames, bytes, sourceSha256: inputHash };
}

async function readRegularPublicFile(root, relative, readBytes = true) {
  const segments = segmentsOf(relative);
  let candidate = root;
  for (let index = 0; index < segments.length; index += 1) {
    candidate = path.join(candidate, segments[index]);
    requireInside(root, candidate);
    const stat = await fs.lstat(candidate);
    if (stat.isSymbolicLink() || (index === segments.length - 1 ? !stat.isFile() : !stat.isDirectory())) {
      throw new Error(`Map-atlas entry is not a regular path: ${relative}`);
    }
  }
  return readBytes ? fs.readFile(candidate) : null;
}

export async function auditPortableTree(root) {
  if (!path.isAbsolute(root)) throw new Error("Portable audit root must be absolute");
  const result = { exists: true, bytes: 0, files: 0, directories: 0, links: 0 };
  async function visit(candidate) {
    requireInside(root, candidate);
    const stat = await fs.lstat(candidate);
    if (stat.isSymbolicLink()) {
      result.links += 1;
      throw new Error(`Portable output retains a link: ${candidate}`);
    }
    if (stat.isFile()) {
      result.bytes += stat.size;
      result.files += 1;
    } else if (stat.isDirectory()) {
      result.directories += 1;
      for (const entry of await fs.readdir(candidate)) await visit(path.join(candidate, entry));
    } else {
      throw new Error(`Unsupported portable output entry: ${candidate}`);
    }
  }
  await visit(root);
  return result;
}

async function assertAbsent(candidate) {
  try {
    await fs.lstat(candidate);
  } catch (error) {
    if (error?.code === "ENOENT") return;
    throw error;
  }
  throw new Error(`Refusing to overwrite an existing portable output: ${candidate}`);
}

export async function copyVerifiedRequiredServerFiles({ sourcePath, destinationPath, sourceBytes }) {
  if (!path.isAbsolute(sourcePath) || !path.isAbsolute(destinationPath) || !Buffer.isBuffer(sourceBytes)) {
    throw new Error("Required-server-files copy needs absolute paths and verified source bytes");
  }
  if (path.basename(sourcePath) !== "required-server-files.json" ||
      path.basename(destinationPath) !== "required-server-files.json") {
    throw new Error("Unexpected required-server-files path");
  }
  const sourceStat = await fs.lstat(sourcePath);
  if (!sourceStat.isFile() || sourceStat.isSymbolicLink()) {
    throw new Error("Source required-server-files must be a regular file");
  }
  await fs.mkdir(path.dirname(destinationPath), { recursive: true });
  try {
    const destinationStat = await fs.lstat(destinationPath);
    if (!destinationStat.isFile() || destinationStat.isSymbolicLink()) {
      throw new Error("Standalone required-server-files must be a regular file");
    }
    const existing = await fs.readFile(destinationPath);
    if (!existing.equals(sourceBytes)) throw new Error("Standalone required-server-files differs from the verified source build");
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    await fs.copyFile(sourcePath, destinationPath, fsConstants.COPYFILE_EXCL);
  }
  const copied = await fs.readFile(destinationPath);
  if (!copied.equals(sourceBytes)) throw new Error("Copied required-server-files differs from the verified source build");
  return JSON.parse(copied.toString("utf8"));
}

function commonRoot(a, b) {
  const aRoot = path.parse(a).root;
  const bRoot = path.parse(b).root;
  if (aRoot.toLowerCase() !== bRoot.toLowerCase()) {
    throw new Error("NFT dependency and standalone roots must share a filesystem root");
  }
  return aRoot;
}

async function routeEntries(sourceAppRoot, distDir) {
  const serverDirectory = path.join(sourceAppRoot, distDir, "server");
  const result = [path.join(sourceAppRoot, "server.js")];
  async function visit(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      const candidate = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`Linked Next route entry: ${candidate}`);
      if (entry.isDirectory()) await visit(candidate);
      else if (entry.isFile() && /(?:^|[\\/])(?:route|page|middleware)\.js$/.test(candidate)) result.push(candidate);
    }
  }
  await visit(serverDirectory);
  return result;
}

/** Sharp's runtimePlatformArch: platform + non-glibc Linux family + architecture. */
export async function collectSharpNativePackages(dependencyRoot) {
  if (!path.isAbsolute(dependencyRoot) || (await fs.lstat(dependencyRoot)).isSymbolicLink()) {
    throw new Error("Sharp dependency root must be an absolute real directory");
  }
  const sharpRoot = path.join(dependencyRoot, "sharp");
  const sharpMetadataPath = path.join(sharpRoot, "package.json");
  if ((await fs.lstat(sharpRoot)).isSymbolicLink() || (await fs.lstat(sharpMetadataPath)).isSymbolicLink()) {
    throw new Error("Linked Sharp source metadata");
  }
  requireInside(dependencyRoot, await fs.realpath(sharpMetadataPath));
  const sharpRequire = createRequire(sharpMetadataPath);
  const sharpMetadata = JSON.parse(await fs.readFile(sharpMetadataPath, "utf8"));
  let libc = "";
  if (process.platform === "linux") {
    const detectRoot = path.join(dependencyRoot, "detect-libc");
    if ((await fs.lstat(detectRoot)).isSymbolicLink()) throw new Error("Linked detect-libc package");
    const detectPath = sharpRequire.resolve(detectRoot);
    requireInside(dependencyRoot, await fs.realpath(detectPath));
    const detectLibc = sharpRequire(detectPath);
    libc = detectLibc.isNonGlibcLinuxSync() ? detectLibc.familySync() : "";
  }
  const targetPlatform = `${process.platform}${libc}-${process.arch}`;
  if (!/^[a-z0-9-]+$/.test(targetPlatform)) throw new Error("Unsafe Sharp target platform");
  const binaryName = `@img/sharp-${targetPlatform}`;
  const companionName = `@img/sharp-libvips-${targetPlatform}`;
  const optional = sharpMetadata.optionalDependencies ?? {};
  if (typeof optional[binaryName] !== "string") {
    throw new Error(`Installed Sharp does not declare target binary ${binaryName}`);
  }
  const names = [binaryName];
  if (typeof optional[companionName] === "string") names.push(companionName);
  const scope = path.join(dependencyRoot, "@img");
  const scopeStat = await fs.lstat(scope);
  if (!scopeStat.isDirectory() || scopeStat.isSymbolicLink()) throw new Error("Linked Sharp scope directory");
  const files = new Set();
  const packages = [];
  for (const name of names) {
    const packageRelative = name.replace("/", path.sep);
    const packageRoot = path.join(dependencyRoot, packageRelative);
    let packageStat;
    try { packageStat = await fs.lstat(packageRoot); }
    catch (error) {
      if (error?.code === "ENOENT") throw new Error(`Missing target Sharp native package: ${name}`, { cause: error });
      throw error;
    }
    if (!packageStat.isDirectory() || packageStat.isSymbolicLink()) {
      throw new Error(`Sharp native package is not a real directory: ${name}`);
    }
    requireInside(dependencyRoot, await fs.realpath(packageRoot));
    let count = 0;
    let bytes = 0;
    const included = new Set();
    async function visit(directory) {
      for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
        const candidate = path.join(directory, entry.name);
        requireInside(packageRoot, candidate);
        const stat = await fs.lstat(candidate);
        if (stat.isSymbolicLink()) throw new Error(`Linked Sharp native entry: ${name}`);
        requireInside(dependencyRoot, await fs.realpath(candidate));
        if (stat.isDirectory()) await visit(candidate);
        else if (stat.isFile()) {
          const relative = path.relative(dependencyRoot, candidate);
          files.add(relative);
          included.add(path.relative(packageRoot, candidate).split(path.sep).join("/"));
          count += 1;
          bytes += stat.size;
        } else throw new Error(`Unsupported Sharp native entry: ${name}`);
      }
    }
    await visit(packageRoot);
    const metadata = JSON.parse(await fs.readFile(path.join(packageRoot, "package.json"), "utf8"));
    if (metadata.name !== name || metadata.version !== optional[name] ||
        !included.has("package.json") || !included.has("LICENSE")) {
      throw new Error(`Sharp native package metadata/license mismatch: ${name}`);
    }
    if (name === binaryName) {
      const exportedBinary = metadata.exports?.["./sharp.node"];
      if (typeof exportedBinary !== "string" || !exportedBinary.startsWith("./") ||
          !included.has(segmentsOf(exportedBinary.slice(2)).join("/")) ||
          !included.has("versions.json")) {
        throw new Error(`Sharp target binary or version metadata missing: ${name}`);
      }
      let nativeBinary = exportedBinary;
      if (exportedBinary.endsWith(".cjs")) {
        const bootstrapPath = path.join(packageRoot, ...segmentsOf(exportedBinary.slice(2)));
        if (exportedBinary !== "./index.cjs" || (await fs.lstat(bootstrapPath)).size > 4096) {
          throw new Error(`Unrecognized Sharp native bootstrap: ${name}`);
        }
        const bootstrap = await fs.readFile(bootstrapPath, "utf8");
        const match = /^\s*module\.exports\s*=\s*require\((["'])(\.\/[A-Za-z0-9_./-]+\.node)\1\);?\s*$/.exec(bootstrap);
        if (!match) throw new Error(`Unrecognized Sharp native bootstrap: ${name}`);
        nativeBinary = match[2];
      }
      if (!nativeBinary.endsWith(".node") ||
          !included.has(segmentsOf(nativeBinary.slice(2)).join("/"))) {
        throw new Error(`Sharp target binary or version metadata missing: ${name}`);
      }
      if (process.platform === "win32" && ![...included].some((file) => file.endsWith(".dll"))) {
        throw new Error(`Sharp Windows package lacks libvips DLLs: ${name}`);
      }
    }
    packages.push({ name, files: count, bytes, destination: `node_modules/${name}` });
  }
  return { targetPlatform, packages, files };
}

export async function materializeStandaloneDependencies({
  sourceAppRoot, destinationAppRoot, sourceStandaloneRoot, sourceDependencyRoot, distDir, trace,
}) {
  for (const value of [sourceAppRoot, destinationAppRoot, sourceStandaloneRoot, sourceDependencyRoot]) {
    if (!path.isAbsolute(value)) throw new Error("Dependency roots must be absolute");
  }
  requireInside(sourceStandaloneRoot, sourceAppRoot);
  const dependencyRoot = await fs.realpath(sourceDependencyRoot);
  if (!(await fs.lstat(dependencyRoot)).isDirectory()) throw new Error("Dependency realpath is not a directory");
  const appDependencyRoot = await fs.realpath(path.join(sourceAppRoot, "node_modules"));
  if (appDependencyRoot !== dependencyRoot) throw new Error("Standalone dependency realpath differs from the verified root");
  const base = commonRoot(sourceAppRoot, dependencyRoot);
  const sourceRequire = createRequire(path.join(sourceAppRoot, "server.js"));
  const requiredEntries = REQUIRED_MODULES.map((moduleId) => sourceRequire.resolve(moduleId));
  const entries = [...new Set([...(await routeEntries(sourceAppRoot, distDir)), ...requiredEntries])];
  const allowed = (candidate) => !isSensitivePath(candidate) &&
    (inside(sourceStandaloneRoot, candidate) || inside(dependencyRoot, candidate));
  const guardedReal = async (candidate) => {
    const absolute = path.resolve(base, candidate);
    if (!allowed(absolute)) return null;
    try {
      const real = await fs.realpath(absolute);
      return allowed(real) ? real : null;
    } catch (error) {
      if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return null;
      throw error;
    }
  };
  const guardedRead = async (candidate) => {
    const real = await guardedReal(candidate);
    if (!real) return null;
    try {
      return await fs.readFile(real);
    } catch (error) {
      if (error?.code === "ENOENT" || error?.code === "EISDIR") return null;
      throw error;
    }
  };
  const nodeFileTrace = trace ?? createRequire(path.join(dependencyRoot, "next", "package.json"))("next/dist/compiled/@vercel/nft").nodeFileTrace;
  const traced = await nodeFileTrace(entries, {
    base, processCwd: sourceAppRoot, mixedModules: true,
    readFile: guardedRead,
    stat: async (candidate) => {
      const real = await guardedReal(candidate);
      return real ? fs.stat(real) : null;
    },
    readlink: async (candidate) => {
      const real = await guardedReal(candidate);
      return real ? fs.readlink(path.resolve(base, candidate)).catch(() => null) : null;
    },
    ignore: (relative) => {
      const candidate = path.resolve(base, relative);
      if (!allowed(candidate)) return true;
      try { return !allowed(realpathSync.native(candidate)); }
      catch { return true; }
    },
  });
  const files = new Set();
  let directoryRecords = 0;
  for (const relative of traced.fileList) {
    const candidate = path.resolve(base, relative);
    if (!inside(sourceStandaloneRoot, candidate) && !inside(dependencyRoot, candidate)) {
      throw new Error(`NFT traced outside the allowed roots: ${candidate}`);
    }
    if (isSensitivePath(candidate)) throw new Error("NFT listed private dependency input");
    let real;
    try { real = await fs.realpath(candidate); }
    catch (error) { throw new Error(`NFT listed a missing dependency: ${relative}`, { cause: error }); }
    if (!inside(dependencyRoot, real)) continue;
    const stat = await fs.stat(real);
    if (stat.isDirectory()) { directoryRecords += 1; continue; }
    if (!stat.isFile()) throw new Error(`NFT listed an unsupported dependency: ${relative}`);
    files.add(path.relative(dependencyRoot, real));
  }
  for (const required of requiredEntries) {
    const real = await fs.realpath(required);
    requireInside(dependencyRoot, real);
    files.add(path.relative(dependencyRoot, real));
  }
  for (const relative of [...files]) {
    const segments = segmentsOf(relative);
    const packageSegments = segments[0]?.startsWith("@") ? segments.slice(0, 2) : segments.slice(0, 1);
    const metadata = path.join(dependencyRoot, ...packageSegments, "package.json");
    try { if ((await fs.lstat(metadata)).isFile()) files.add(path.relative(dependencyRoot, metadata)); }
    catch (error) { if (error?.code !== "ENOENT") throw error; }
  }
  const nativeClosure = await collectSharpNativePackages(dependencyRoot);
  for (const relative of nativeClosure.files) files.add(relative);
  const destinationDependencyRoot = path.join(destinationAppRoot, "node_modules");
  await assertAbsent(destinationDependencyRoot);
  await fs.mkdir(destinationDependencyRoot);
  let bytes = 0;
  let nativeBindings = 0;
  for (const relative of [...files].sort()) {
    const source = path.resolve(dependencyRoot, ...segmentsOf(relative));
    const destination = path.resolve(destinationDependencyRoot, ...segmentsOf(relative));
    requireInside(dependencyRoot, source);
    requireInside(destinationDependencyRoot, destination);
    const real = await fs.realpath(source);
    requireInside(dependencyRoot, real);
    const stat = await fs.stat(real);
    if (!stat.isFile()) throw new Error(`Dependency is not a regular file: ${relative}`);
    await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.copyFile(real, destination, fsConstants.COPYFILE_EXCL);
    bytes += stat.size;
    if (relative.endsWith(".node")) nativeBindings += 1;
  }
  const destinationRequire = createRequire(path.join(destinationAppRoot, "server.js"));
  for (const moduleId of REQUIRED_MODULES) {
    const resolved = destinationRequire.resolve(moduleId);
    requireInside(destinationDependencyRoot, resolved);
  }
  const nativeBinaryName = `@img/sharp-${nativeClosure.targetPlatform}`;
  for (const moduleId of [`${nativeBinaryName}/sharp.node`, `${nativeBinaryName}/versions`]) {
    const resolved = destinationRequire.resolve(moduleId);
    requireInside(destinationDependencyRoot, resolved);
  }
  for (const nativePackage of nativeClosure.packages) {
    const metadata = path.join(destinationDependencyRoot, ...nativePackage.name.split("/"), "package.json");
    requireInside(destinationDependencyRoot, metadata);
    if (!(await fs.lstat(metadata)).isFile()) throw new Error(`Copied Sharp package metadata missing: ${nativePackage.name}`);
  }
  const warningCodes = Object.create(null);
  const warningDetails = [];
  for (const warning of traced.warnings ?? []) {
    const code = String(warning?.code ?? warning?.name ?? "unknown");
    const message = String(warning?.message ?? "");
    warningCodes[code] = (warningCodes[code] ?? 0) + 1;
    // Keep trace diagnostics without serializing Error stacks or unbounded messages.
    warningDetails.push({ code, message: message.slice(0, 8192), messageTruncated: message.length > 8192 });
  }
  return { entries: entries.length, files: files.size, bytes, nativeBindings, directoryRecords,
    targetPlatform: nativeClosure.targetPlatform, nativePackages: nativeClosure.packages,
    nativePackageFiles: nativeClosure.packages.reduce((sum, entry) => sum + entry.files, 0),
    nativePackageBytes: nativeClosure.packages.reduce((sum, entry) => sum + entry.bytes, 0),
    warningCount: warningDetails.length, warningCodes, warningDetails };
}
