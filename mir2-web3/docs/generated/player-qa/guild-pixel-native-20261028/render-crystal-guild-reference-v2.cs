using System;
using System.IO;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;
using System.Windows.Forms;
using System.Collections.Generic;
using System.Web.Script.Serialization;

// Standalone GDI reference, not a replacement client or a gameplay run.
// MirLabel.CreateTexture draw settings and the five TextRenderer calls copied
// from Crystal/Client/MirControls/MirLabel.cs, preserving default GDI padding.
public static class CrystalGuildReferenceV2 {
  static readonly List<object> rows = new List<object>();
  static string output;
  static Rectangle? Bounds(Bitmap b, bool nonBlack) {
    int l=b.Width,t=b.Height,r=-1,d=-1;
    for(int y=0;y<b.Height;y++)for(int x=0;x<b.Width;x++){
      Color c=b.GetPixel(x,y);if(c.A==0||(nonBlack&&c.R==0&&c.G==0&&c.B==0))continue;
      l=Math.Min(l,x);t=Math.Min(t,y);r=Math.Max(r,x);d=Math.Max(d,y);
    }
    return r<l?(Rectangle?)null:Rectangle.FromLTRB(l,t,r+1,d+1);
  }
  static object Rect(Rectangle? r){return r.HasValue?(object)new {x=r.Value.X,y=r.Value.Y,width=r.Value.Width,height=r.Value.Height}:null;}
  static Bitmap Label(string id,string text,int w,int h,TextFormatFlags flags,Color fore,int x,int y){
    Bitmap image=new Bitmap(w,h,PixelFormat.Format32bppArgb);image.SetResolution(96,96);
    using(Graphics graphics=Graphics.FromImage(image))using(Font font=new Font("Arial",8F,FontStyle.Regular,GraphicsUnit.Point)){
      graphics.SmoothingMode=SmoothingMode.AntiAlias;
      graphics.TextRenderingHint=TextRenderingHint.AntiAliasGridFit;
      graphics.CompositingQuality=CompositingQuality.HighQuality;
      graphics.InterpolationMode=InterpolationMode.NearestNeighbor;
      graphics.PixelOffsetMode=PixelOffsetMode.HighQuality;
      graphics.TextContrast=0;graphics.Clear(Color.Transparent);
      TextRenderer.DrawText(graphics,text,font,new Rectangle(1,0,w,h),Color.Black,flags);
      TextRenderer.DrawText(graphics,text,font,new Rectangle(0,1,w,h),Color.Black,flags);
      TextRenderer.DrawText(graphics,text,font,new Rectangle(2,1,w,h),Color.Black,flags);
      TextRenderer.DrawText(graphics,text,font,new Rectangle(1,2,w,h),Color.Black,flags);
      TextRenderer.DrawText(graphics,text,font,new Rectangle(1,1,w,h),fore,flags);
      rows.Add(new{id,text,kind="MirLabel",windowX=x,windowY=y,width=w,height=h,fontFamily=font.Name,fontPt=font.Size,
        dpiX=graphics.DpiX,dpiY=graphics.DpiY,fontHeight=font.Height,drawFormat=flags.ToString(),defaultGdiPaddingRetained=true,
        foregroundDrawRectangle=new{x=1,y=1,width=w,height=h},outlineAlphaBounds=Rect(Bounds(image,false)),foregroundNonBlackBounds=Rect(Bounds(image,true)),file=id+".png"});
    }
    image.Save(Path.Combine(output,id+".png"),ImageFormat.Png);return image;
  }
  static void Sprite(Graphics g,string assetRoot,string library,int index,int x,int y){
    string file=Path.Combine(assetRoot,library,index+".png");using(Bitmap b=new Bitmap(file))g.DrawImageUnscaled(b,x,y);
  }
  static void Input(string text){
    using(TextBox box=new TextBox()){
      box.BackColor=Color.FromArgb(255,25,25,25);box.BorderStyle=BorderStyle.None;
      box.Font=new Font("Arial",10F*96F/96F);box.ForeColor=Color.White;box.Size=new Size(130,21);box.MaxLength=20;box.Text=text;
      box.CreateControl();using(Bitmap b=new Bitmap(130,21,PixelFormat.Format32bppArgb)){
        b.SetResolution(96,96);box.DrawToBitmap(b,new Rectangle(0,0,130,21));
        string id=text.Length==0?"recruit-input-empty-10pt":"recruit-input-hahah-10pt";b.Save(Path.Combine(output,id+".png"),ImageFormat.Png);
        rows.Add(new{id,text,kind="MirTextBox",windowX=395,windowY=360,requestedWidth=130,requestedHeight=21,
          actualTextBoxWidth=box.Width,actualTextBoxHeight=box.Height,fontFamily=box.Font.Name,fontPt=box.Font.Size,fontHeight=box.Font.Height,
          borderStyle=box.BorderStyle.ToString(),autoSize=box.AutoSize,multiline=box.Multiline,draw="WinForms TextBox.DrawToBitmap; no fake MirLabel text",file=id+".png"});
      }
    }
  }
  [STAThread] public static int Main(string[] args){
    if(args.Length!=2)throw new ArgumentException("ASSET_ROOT NEW_OUTPUT_DIRECTORY");
    output=Path.GetFullPath(args[1]);if(Directory.Exists(output))throw new IOException("Exclusive output directory required");Directory.CreateDirectory(output);
    string statusExpBarPath=Path.GetFullPath(Path.Combine(args[0],"Prguse2","424.png"));
    byte[] statusExpBarBytes=File.ReadAllBytes(statusExpBarPath);
    string statusExpBarSha;
    using(System.Security.Cryptography.SHA256 hash=System.Security.Cryptography.SHA256.Create())
      statusExpBarSha=BitConverter.ToString(hash.ComputeHash(statusExpBarBytes)).Replace("-","").ToLowerInvariant();
    using(Bitmap bar=new Bitmap(statusExpBarPath))
      if(bar.Width!=260||bar.Height!=22)throw new InvalidDataException("Expected original Prguse2[424] 260x22 frame");
    Application.SetCompatibleTextRenderingDefault(false);
    using(Bitmap composed=new Bitmap(590,432,PixelFormat.Format32bppArgb))using(Graphics g=Graphics.FromImage(composed)){
      composed.SetResolution(96,96);g.Clear(Color.Transparent);
      Sprite(g,args[0],"Prguse",180,0,0);Sprite(g,args[0],"Title",25,18,9);
      Sprite(g,args[0],"Title",94,20,38);Sprite(g,args[0],"Title",99,91,38);Sprite(g,args[0],"Title",105,162,38);Sprite(g,args[0],"Title",101,233,38);
      Sprite(g,args[0],"Title",95,430,38);Sprite(g,args[0],"Title",104,501,38);Sprite(g,args[0],"Prguse2",360,565,4);
      Sprite(g,args[0],"Prguse",1850,365,62);Sprite(g,args[0],"Title",356,525,358);
      using(Bitmap b=Label("statistics-headers","Guild Name\n\nLevel\n\nMembers",75,300,TextFormatFlags.Right,Color.Gray,362,107))g.DrawImageUnscaled(b,362,107);
      using(Bitmap b=Label("guild-hahah","hahah",120,200,TextFormatFlags.WordBreak,Color.White,437,107))g.DrawImageUnscaled(b,437,107);
      using(Bitmap b=Label("level-0","0",120,200,TextFormatFlags.WordBreak,Color.White,437,133))g.DrawImageUnscaled(b,437,133);
      using(Bitmap b=Label("members-1-of-5","1/5",120,200,TextFormatFlags.WordBreak,Color.White,437,159))g.DrawImageUnscaled(b,437,159);
      using(Bitmap b=Label("recruit-label","Recruit Member",150,15,TextFormatFlags.WordBreak,Color.White,391,343))g.DrawImageUnscaled(b,391,343);
      // Original GuildDialog.StatusExpBar_BeforeDraw, before the separate label.
      Sprite(g,args[0],"Prguse2",424,322,403);
      using(Bitmap b=Label("experience-0-percent","0%",260,15,TextFormatFlags.VerticalCenter|TextFormatFlags.HorizontalCenter,Color.White,322,405))g.DrawImageUnscaled(b,322,405);
      composed.Save(Path.Combine(output,"guild-notice-statistics-label-reference.png"),ImageFormat.Png);
    }
    Input("");Input("hahah");
    File.WriteAllText(Path.Combine(output,"glyphbounds.json"),new JavaScriptSerializer().Serialize(new{
      schemaVersion=2,referenceOnly=true,gameplayPerformed=false,dpi=96,
      statusExpBarAsset=new{file=statusExpBarPath,bytes=statusExpBarBytes.Length,sha256=statusExpBarSha,library="Prguse2",frame=424,width=260,height=22,windowX=322,windowY=403,drawnBeforeZeroPercentLabel=true},font="Arial; explicit reference condition, actual Settings.ini may override",
      mirLabelAutoSize=false,outline=true,originalForegroundOffset=new{x=1,y=1},
      composedScope="Original exported bitmap layers including Prguse2[424] at (322,403), and exact MirLabel references only; recruit TextBox separately exported, not hand drawn. Conditional tab visibility fixed to screenshot description.",
      notPixelAcceptance="No current native screenshot or original running Crystal screenshot compared; original DX/GPU final composition is not executed",labels=rows}));
    Console.WriteLine("Reference render complete: "+output);return 0;
  }
}
