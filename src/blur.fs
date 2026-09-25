#version 330

in vec2 fragTexCoord;
in vec4 fragColor;

uniform sampler2D texture0;
uniform int blurRadius;
uniform float alpha;

out vec4 finalColor;
void main()
{
    vec2 texel = 1.0 / vec2(textureSize(texture0, 0));

    vec4 color = vec4(0.0);
    float samples = 0.0;

    for (int y = -blurRadius; y <= blurRadius; y++){
        for (int x = -blurRadius; x <= blurRadius; x++){
            color += texture( texture0, fragTexCoord + texel * vec2(float(x), float(y)));
            samples += 1.0;
        }
    }

    vec4 result = color / samples;

    result.a *= alpha / 100.0;
    
    if (result.g > 0.5 && result.g > result.r && result.g > result.b) {
        result.a = 0.0;
    }

    finalColor = result;
}