import sys
import torch
import warnings
warnings.filterwarnings("ignore")

def main():
    if len(sys.argv) < 3:
        print("Usage: python3 generate_image.py <prompt> <output_path>")
        sys.exit(1)
        
    prompt = sys.argv[1]
    output_path = sys.argv[2]
    
    try:
        from diffusers import FluxPipeline
    except ImportError:
        print("CRITICAL FATAL SYSTEM ERROR: diffusers library not found. Please install diffusers and torch.")
        sys.exit(1)

    device = "mps" if torch.backends.mps.is_available() else "cpu"
    dtype = torch.bfloat16
    
    print(f"Loading FLUX.1-dev on {device}...", file=sys.stderr)
    try:
        pipe = FluxPipeline.from_pretrained(
            "black-forest-labs/FLUX.1-dev",
            torch_dtype=dtype
        )
        pipe.to(device)
    except Exception as e:
        print(f"CRITICAL FATAL SYSTEM ERROR: Failed to load Flux pipeline. {e}")
        sys.exit(1)
    
    print(f"Generating image for: {prompt}", file=sys.stderr)
    try:
        image = pipe(
            prompt,
            guidance_scale=3.5,
            num_inference_steps=50, # Following Ernos 3.0 config
            width=1024,
            height=1024,
            max_sequence_length=512,
            generator=torch.Generator("cpu").manual_seed(0)
        ).images[0]
        
        image.save(output_path)
        print(f"Successfully saved to {output_path}")
    except Exception as e:
        print(f"CRITICAL FATAL SYSTEM ERROR: Inference failed. {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
